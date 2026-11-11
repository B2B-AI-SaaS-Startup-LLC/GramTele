# tg-runtime

Порт `Telegram/lib_crl`: главная очередь, фоновый пул, последовательные очереди,
объекты на очередях, guard, монотонное время, семафор.

Удобнее всего подключать под старым именем, тогда вызовы почти не меняются:

```rust
use tg_runtime as crl;
```

## Модель

- Один главный поток (ядро и интерфейс) плюс пул фоновых потоков.
- Фон идёт через **blocking-пул tokio** (приватный рантайм, `Executor`). Задачи crl
  это обычные синхронные замыкания, которые могут блокироваться (диск, семафор,
  `sync`). На async-воркерах они морили бы реактор, а blocking-пул растёт по требованию
  (до 512 потоков), так что заблокированная задача не мешает остальным.
- Главный поток представлен `MainQueue`. Она ничего не знает о цикле: цикл даёт ей
  будильник (`init_main_queue`) и сам зовёт `process_main_queue()`. Для тестов и консоли
  есть `MainLoop`, который крутит очередь в текущем потоке.
- Глобальная очередь одна (`main_queue()`); для тестов можно создавать свои `MainQueue::new()`.

## Таблица «как писали в C++ → как писать в Rust»

| C++ (пример из клиента) | Rust |
|---|---|
| `crl::time`, `kTimeout = 10 * crl::time(1000)` (sandbox.cpp) | `crl::Time` (`i64`, мс); `10 * 1000_i64` |
| `crl::now()` (`deadline = crl::now() + kCleanupIpcTimeout`) | `crl::now()` |
| `crl::profile()` (debug_log.cpp) | `crl::profile()` (`ProfileTime`, мкс) |
| `crl::adjust_time()` (timer.cpp, application.cpp) | `crl::adjust_time()` |
| `crl::async([=] { ... })` (animated_icon.cpp) | `crl::async_(move \|\| { ... })`, замыкание `Send + 'static` |
| `crl::async([guard = shared_from_this()] { guard->render(); })` | `let g = Arc::clone(&self.impl_); crl::async_(move \|\| g.render())` |
| `crl::sync([&] { ... })` | `let r = crl::sync(move \|\| ...)` возвращает `Option<R>`, значения возвращаются, а не пишутся по ссылке |
| `crl::on_main([=] { ... })` | `crl::on_main(move \|\| ...)` (`Send`, из любого потока) |
| `crl::on_main(this, [=] { checkForQuit(); })` (sandbox.cpp:194), вызов из главного потока | `crl::on_main_local_guarded(&self.guardian, move \|\| ...)`, замыкание может быть не `Send` |
| `crl::on_main(weak, [raw, weak] { ... })` (calls_instance.cpp), вызов из фона | `main_ref.on_main(move \|obj\| ...)`, где `main_ref: MainRef<T>` создан на главном потоке |
| `crl::on_main_sync([&] { ... })` (ayu_sync.cpp) | `crl::on_main_sync(move \|\| ...)` возвращает `Option<R>`; не из главного потока (паника вместо дедлока) |
| `crl::guard(this, [=] { ... })` (483 места) | `crl::guard_fn(&self.guardian, move \|\| ...)` (или `guard`, `guard_mut`, `guard_fn1`, `guard_fn2`, `guard_mut1` по виду замыкания) |
| `crl::guard(weak, [=](int approved) { ... })` (share_box.cpp:700) | `crl::guard_fn1(&weak, move \|approved: i32\| ...)`, `weak` это `Weak<T>`, `rc::Weak<T>`, `Arc<T>`, `Rc<T>`, `Guardian`, `Guard`, `MainRef<T>` |
| `crl::guard_traits<base::weak_ptr<T>>` (lib_base) | `impl crl::AsGuard for WeakPtr<T>` в `tg-base` (`as_guard` + `GuardToken::alive`) |
| `crl::on_main_queue(this, { [=]{ a(); }, [=]{ b(); } }).dispatch()` (main_session.cpp:243) | `crl::on_main_queue_local(&guard, vec![Box::new(a), Box::new(b)]).dispatch()`; для `Send`-шагов `on_main_queue` |
| `crl::init_main_queue(PushToMainQueueGeneric)` (main_queue_processor.cpp) | `crl::init_main_queue(move \|\| post_wake_event())` |
| `crl::wrap_main_queue(...)` (enterFromEventLoop) | `crl::wrap_main_queue(\|run\| enter_from_event_loop(run))` |
| `crl::on_main_update_requests()` (rpl::producer) | `let sub = crl::on_main_update_requests(\|\| ...)` (подписка снимается `drop`); окно вызывает `crl::fire_main_update_requests()`. Обёртка в `rpl::producer` делается в `tg-rpl`/`tg-app` |
| `crl::queue q; q.async([=]{...}); q.sync([&]{...})` | `let q = crl::Queue::new(); q.async_(move \|\| ...); q.sync(move \|\| ...)` |
| `crl::semaphore s; s.acquire(); s.release();` (streaming) | `crl::Semaphore`; делится через `Arc<Semaphore>` |
| `_wrapped.with([&](Implementation &) { semaphore.release(); }); semaphore.acquire();` (Database::sync) | `let sem = Arc::new(Semaphore::new()); let s = sem.clone(); wrapped.with(move \|_\| s.release()); sem.acquire();` или проще `wrapped.with_sync(\|_\| ())` |
| `crl::object_on_queue<Implementation> _wrapped;` + `_wrapped(args...)` | `crl::ObjectOnQueue<Implementation>`; `ObjectOnQueue::new(move \|\| Implementation::new(args))` (создаётся на очереди) или `from_value(v)` |
| конструктор `Implementation(crl::weak_on_queue<Implementation> weak, ...)` (VideoTrackObject) | `ObjectOnQueue::new_with_weak(move \|weak\| Implementation::new(weak, ...))` |
| `_wrapped.with([=](Implementation &unwrapped) { ... })` | `wrapped.with(move \|unwrapped\| ...)`: всегда `&mut T`, различия const/неconst нет |
| `_wrapped.with_sync(...)` | `wrapped.with_sync(move \|u\| ...)` возвращает `Option<R>` |
| `crl::weak_on_queue<T> _weak; _weak.with(...)` | `crl::WeakOnQueue<T>`, `Clone`, `Default`, `Send`; `weak.with(...)`, задача отбрасывается, если владелец удалён |
| `weak.runner()` | `weak.runner().run(move \|\| ...)` |
| `weak.destroy(std::move(value))` | `weak.destroy(value)`: `value` уничтожается на очереди |
| `static auto queue = crl::object_on_queue<HunspellService>();` | `static Q: OnceLock<ObjectOnQueue<HunspellService>>`; `Q.get_or_init(\|\| ObjectOnQueue::new(HunspellService::default))` |
| `crl::object_on_thread<T>`, `crl::weak_on_thread<T>` | `crl::ObjectOnThread<T>`, `crl::WeakOnThread<T>` (свой поток на объект) |
| `_wrapped.producer_on_main(...)` / `producer(...)` | **отложено** до готовности `tg-rpl` |
| `crl::toggle_fp_exceptions(true)` | `crl::toggle_fp_exceptions(true)`: пустая функция |

### Как это переписывать: `[&]` и не-Send

- Замыкания, уходящие на другой поток, обязаны быть `Send + 'static`. Захват по ссылке
  (`[&]`) надо заменить на `Arc`/канал или на возвращаемое значение `with_sync`/`sync`.
- Объект главного потока (не `Send`) в фоновую задачу не передаётся. Вместо него в задачу
  кладут `MainRef<T>` (id, `Copy + Send + Sync`) или `Guard`. Саму работу с объектом делают
  в замыкании `main_ref.on_main(|obj| ...)`, которое получит `&Rc<T>` на главном потоке
  и только если объект ещё жив.

## Как устроен guard

`crl::guard(this, f)` в C++ проверяет weak-указатель непосредственно перед вызовом. В Rust:

1. **`AsGuard` / `GuardToken`**: аналог `guard_traits`. Токен это слабая ссылка, её
   проверяют в момент вызова. Реализовано для `Weak`, `rc::Weak`, `Arc`, `Rc`, `Guardian`,
   `Guard`, `MainRef`. Новые типы подключаются реализацией `AsGuard` у владельца типа.
2. **`Guardian`**: поле, которое кладут в объект интерфейса (аналог `has_weak_ptr`).
   `guardian.guard()` даёт `Guard` (`Send + Sync + Clone`). Сам объект может быть не `Send`.
   Guard говорит только «объект жив»; обратиться к объекту он не позволяет.
3. **`MainRef<T>`**: идентификатор `Rc<T>`, зарегистрированного в потоковом реестре главного
   потока. Id можно держать в фоне. Реестр хранит `Weak`, id не переиспользуются.

Ограничения: `MainRef` требует `Rc<T>` и регистрации на главном потоке; живость проверяется
только на нём (в других потоках `alive()` возвращает `false`). Для кода, который идёт в
единой архитектуре «хранилище + типизированные id» из PORTING.md, роль `MainRef` играет
сам идентификатор хранилища, а `Guardian` не нужен.

Мёртвый guard делает вызов пустым и возвращает `Default::default()` (в C++ `Return()`).

## Гарантии порядка и потокобезопасности

Главная очередь (взято из общей реализации оригинала, проверено тестами):

- `on_main` никогда не выполняет задачу сразу, даже из главного потока.
- FIFO. Один вызов `process()` выполняет ровно те задачи, что были в очереди к его началу.
  Задачи, поставленные во время разбора (в том числе вложенные `on_main`), выполняются
  следующим `process()`.
- Будильник вызывается один раз при переходе «пусто → есть работа» и ещё раз после пачки,
  если за время разбора пришла новая работа.
- `process()` можно вызвать повторно изнутри задачи (вложенные циклы). Вложенный вызов
  берёт только более новые задачи, поэтому они выполнятся раньше остатка внешней пачки.
  Это поведение оригинала, оно оставлено и покрыто тестом.
- Порядок между потоками: задачи одного потока идут в порядке постановки; между потоками
  порядок определяет момент попадания в очередь.
- `on_main_local` (не `Send`) и `on_main` делят одну очередь и один порядок. Вызывать
  `on_main_local` можно только из главного потока.
- Паника в задаче: остальные задачи пачки всё равно выполняются, потом паника
  пробрасывается из `process()`.

Фоновая очередь `Queue`:

- Задачи выполняются строго по одной и по порядку, но не обязательно на одном потоке
  (как в оригинале). Все накопленные задачи разбираются одной пачкой на одном потоке пула.
- `sync` ждёт всё поставленное ранее. Вызов `sync` из задачи той же очереди паникует
  вместо дедлока.
- Паника в задаче печатается хуком, очередь продолжает работать.

`ObjectOnQueue` / `ObjectOnThread`:

- Конструктор выполняется на очереди первой задачей, вызовы `with` приходят по порядку и
  по одному. `T: Send`, `Sync` не нужен.
- Деструктор `T` выполняется на очереди после всех уже поставленных задач. Задачи,
  поставленные через `WeakOnQueue` до удаления владельца, выполнятся.
- Через `WeakOnQueue` после удаления объекта задача молча отбрасывается.
- `with_sync` изнутри задачи той же очереди даёт дедлок, как в оригинале.

`Semaphore`: бинарный и насыщающийся (два `release` подряд дают одно разрешение), как
общий вариант оригинала.

## Завершение

- `MainQueue::shutdown()` (аналог уничтожения очереди в оригинале): невыполненные задачи
  отбрасываются без выполнения (их захваты освобождаются), последующие `on_main` молча
  игнорируются, будильник снимается. Не-`Send` задачи освобождаются, если вызвать
  `shutdown` на главном потоке. Если `shutdown` вызван из задачи, остаток текущей пачки
  отбрасывается (аналог проверки `_alive`).
- `Executor::shutdown()`: пул перестаёт принимать работу; выполняющиеся задачи
  доработают, не начатые отбрасываются; `async_` становится пустой операцией, `sync` и
  `Queue::sync` возвращают `None`. Это не требуется при выходе из процесса: глобальный
  пул не присоединяется, невыполненные фоновые задачи при выходе пропадают, как и в оригинале.
- До `init_main_queue` задачи не теряются (в оригинале терялись), а ждут первого `process()`.

## Время

`Time` это `i64` миллисекунды, `ProfileTime` микросекунды. Отсчёт идёт от первого
обращения к часам (в оригинале от статической инициализации библиотеки); вызовите
`crl::init_time()` в начале `main`, чтобы приблизить это к старту процесса.
Часы монотонные (`Instant`), поправка на сон системы делается `adjust_time()` по формуле оригинала.
