# tg-rpl

Порт `Telegram/lib_rpl` (реактивная библиотека Telegram Desktop) на Rust. На ней построен почти весь клиент: `rpl::producer`, `rpl::event_stream`, `rpl::variable`, `rpl::lifetime`. Этот документ — справочник для переводчиков: как механически переписать код оригинала на этот крейт.

```rust
use tg_rpl as rpl;
```

## Главные правила перевода

1. **`|` заменяется вызовом метода.** `a | rpl::map(f) | rpl::filter(g)` → `a.map(f).filter(g)`. Все операторы `lib_rpl` — методы `Producer`. Собственные операторы приложения (функторы, которые в C++ пишутся через `|`, например `| Ui::Text::ToUpper()`) вызываются через `.pipe(...)`.
2. **Подписка с lifetime сохраняет имя из C++, подписка, возвращающая lifetime, получает суффикс `_returning`.** `| rpl::on_next(f, lifetime)` → `.on_next(f, &lifetime)`; `auto l = p | rpl::on_next(f);` → `let l = p.on_next_returning(f);`. То же для `start`, `start_copy`, `start_existing`, `on_error`, `on_done`, `on_next_error`, `on_next_done`, `on_error_done`, `on_next_error_done`.
3. **Обработчик всегда получает значение по значению.** C++ вызывает обработчик без аргументов (`[=] { ... }`), с одним аргументом или с распакованным кортежем (`[=](int a, int b)`). В Rust: `|_| ...`, `|value| ...`, `|(a, b)| ...`.
4. **`rpl::mappers` (`_1`, `_2`, `!_1`, `_1 + _2`, `_1_of_two`, `tuple(...)`) не портируются.** Пишется замыкание: `rpl::map(_1 > 0)` → `.map(|v| v > 0)`.
5. **Значения в `fire`, `single`, `combine`, `Variable` должны быть `Clone`** (в C++ — копируемыми). Тип ошибки почти везде `NoError`, его не пишут: `Producer<i32>` = `rpl::producer<int>`, `Producer` = `rpl::producer<>` (значение `()`).
6. **Захват `this` и ссылок `[&]` заменяется `Weak`/идентификатором.** Замыкания обязаны быть `'static`. Захват сильного `Rc` объекта, который сам владеет lifetime подписки, — цикл ссылок и утечка; так же, как цикл `shared_ptr` в C++.

## Таблица соответствий

| C++ | Rust | Пример из клиента (C++) |
|---|---|---|
| `rpl::producer<T>` / `rpl::producer<>` | `Producer<T>` / `Producer` | `rpl::producer<> fooRequests() const;` → `fn foo_requests(&self) -> Producer` |
| `rpl::producer<T, Error>` | `Producer<T, Error>` | `rpl::producer<rpl::no_value, Usernames::Error>` → `Producer<NoValue, UsernamesError>` |
| `rpl::producer<T>([=](auto consumer) { ...; return lifetime; })` | `Producer::new(move \|consumer\| { ...; lifetime })` | `history_view_pinned_bar.cpp:179` `rpl::make_producer<Ui::MessageBarContent>([=](auto consumer) {` |
| `rpl::make_producer<T>(gen)` | `rpl::make_producer(gen)` или `Producer::<T>::new(gen)` | `dialogs_stories_content.cpp:135` |
| `return [=] { cleanup(); };` (из генератора) | `Lifetime::from_fn(move \|\| cleanup())` | `producer_tests.cpp` |
| `return rpl::lifetime();` | `Lifetime::new()` | повсеместно |
| `consumer.put_next(v)` / `put_next_copy(v)` / `put_next_forward(v)` | `consumer.put_next(v)` / `consumer.put_next_copy(&v)` / `consumer.put_next(v)` | `api_user_names.cpp:222` `consumer.put_done()` |
| `consumer.put_error(e)` / `put_done()` | `consumer.put_error(e)` / `consumer.put_done()` | то же |
| `consumer.lifetime()`-подобное: `consumer.add_lifetime(l)` | `consumer.add_lifetime(l)` | `then.h` |
| `p \| rpl::on_next([=](T v) {...}, lifetime())` | `p.on_next(move \|v\| {...}, &self.lifetime)` | `tray.cpp:52` `trayIconMonochromeChanges() \| rpl::on_next([=] { updateIconCounters(); }, _tray.lifetime());` → `.on_next(move \|_\| ..., tray.lifetime())` |
| `auto l = p \| rpl::on_next(f);` | `let l = p.on_next_returning(f);` | `variable_tests.cpp` |
| `p \| rpl::on_next_done(f, d, lt)` / `on_next_error(f, e, lt)` / `on_error_done(e, d, lt)` / `on_next_error_done(f, e, d, lt)` / `on_done(d, lt)` / `on_error(e, lt)` | одноимённые методы, lifetime по ссылке | `producer_tests.cpp` «on_*» |
| `std::move(p).start(n, e, d, lifetime)` | `p.start(n, e, d, &lifetime)` | `flatten_latest.h` |
| `auto l = std::move(p).start(n, e, d);` | `let l = p.start_returning(n, e, d);` | `then.h` |
| `p.start_copy(n, e, d, lifetime)` | `p.start_copy(n, e, d, &lifetime)` (`&self`) | `producer_tests.cpp` |
| `std::move(p).start_existing(consumer)` | `p.start_existing_returning(&consumer)` | `deferred.h` |
| `p \| rpl::start(lifetime)` | `p.start_no_handlers(&lifetime)` | 3 места |
| `rpl::lifetime _lifetime;` | `lifetime: Lifetime` | `connection_tcp.h:97` |
| `lifetime.add([=] {...})` | `lifetime.add(move \|\| {...})` | повсеместно |
| `lifetime.add(std::move(other))` | `lifetime.add_lifetime(other)` | `lifetime.h` |
| `lifetime.destroy()` | `lifetime.destroy()` (метод `&self`) | `connection_tcp.cpp:502` `_connectedLifetime.destroy();` |
| `if (lifetime)` | `if !lifetime.is_empty()` | — |
| `const auto s = lifetime.make_state<State>(args)` | `let s = lifetime.make_state(State::new(args));` затем `s.get().field` | `tray_accounts_menu.cpp:25` `lifetime.make_state<rpl::lifetime>()` → `let inner = lifetime.make_state(Lifetime::new());` |
| `state->x = 5;` (state из `make_state`) | поле `Cell`/`RefCell`: `s.get().x.set(5)` | `api_confirm_phone.cpp:92` |
| `rpl::event_stream<T> _changes;` | `changes: EventStream<T>` | `mtp_instance.cpp:241` `rpl::event_stream<DcId> _dcTemporaryKeyChanged;` |
| `_stream.fire(v)` / `fire({})` / `fire_copy(v)` | `stream.fire(v)` / `stream.fire(())` / `stream.fire_copy(&v)` | `tray.cpp:72` `_minimizeMenuItemClicks.fire({});` → `.fire(())`; `api_blocked_peers.cpp:95` `_changes.fire_copy(*_slice);` |
| `_stream.fire_error(e)` / `fire_done()` | `stream.fire_error(e)` / `stream.fire_done()` | — |
| `return _stream.events();` | `self.stream.events()` | `mtp_instance.cpp` |
| `_stream.events_starting_with({})` | `stream.events_starting_with(())` | `tray.cpp:119` `_textUpdates.events_starting_with({})` |
| `_stream.has_consumers()` | `stream.has_consumers()` | `gift_auctions.cpp:99` |
| `p \| rpl::start_to_stream(_stream, _lifetime)` | `p.start_to_stream(&self.stream, &self.lifetime)` | `mainwidget.cpp:241` `removeRequests() \| rpl::start_to_stream(_thirdSectionRemoveRequests, _lifetime);` |
| `auto s = p \| rpl::start_spawning(box->lifetime());` | `let s = p.start_spawning(box.lifetime());` | `translate_box_content.cpp:112` |
| `rpl::variable<bool> _x = false;` | `x: Variable<bool>` = `Variable::new(false)` | `main_session.h:356` `rpl::variable<bool> _premiumPossible = false;` |
| `rpl::variable<T>` для типа без `operator==` | `Variable::new_non_comparable(v)` | `main_session.h:365` (если у `FreezeInfo` нет `==`) |
| `_x = value;` | `x.set(value)` | повсеместно |
| `_x = std::move(producer);` / `rpl::variable<T>(producer)` | `x.set_producer(p)` / `Variable::from_producer(p)` | — |
| `_x.force_assign(v)` | `x.force_assign(v)` | `api_chat_invite.cpp:354` `state->loading.force_assign(false);` |
| `_x.current()` | `x.current()` (копия) или `x.current_ref()` (`Ref`, не держать через присваивание) | повсеместно |
| `_x.value()` / `_x.changes()` | `x.value()` / `x.changes()` | `main_session.cpp:437` `_premiumPossible.value()` |
| `_x.reset(v)` / `reset()` / `reset_with_error(e)` | `x.reset(v)` / `x.reset_default()` / `x.reset_with_error(e)` | — |
| `a = b;` (копирование variable) | `a.set(b.current())` | — |
| `rpl::single(v)` / `rpl::single()` / `rpl::single(rpl::empty)` | `rpl::single(v)` / `rpl::single(())` / `rpl::single(rpl::EMPTY)` | 1512 вызовов |
| `rpl::single<T, Error>(v)` / `rpl::never<T, Error>()` / `rpl::complete<T, Error>()` | `rpl::single_e::<T, Error>(v)` / `rpl::never_e::<T, Error>()` / `rpl::complete_e::<T, Error>()` | `rpl::never<rpl::no_value, Error>()` |
| `rpl::never<>()` / `rpl::never<T>()` | `rpl::never::<()>()` / `rpl::never::<T>()` | `mainwidget.cpp:209` `return rpl::never<>();` |
| `rpl::complete<T>()` / `rpl::fail<T>(e)` | `rpl::complete::<T>()` / `rpl::fail::<T, _>(e)` | — |
| `rpl::vector(std::move(v))` / `rpl::range(r)` / `rpl::ints(a, b)` / `rpl::ints(n)` | `rpl::vector(v)` / `rpl::range(r)` / `rpl::ints(a, b)` / `rpl::ints_count(n)` | — |
| `rpl::duplicate(p)` | `rpl::duplicate(&p)` или `p.clone()` | `api_chat_filters.cpp:781` `rpl::duplicate(badge)` |
| `p \| rpl::map([=](T v) { return f(v); })` | `p.map(move \|v\| f(v))` | 1409 вызовов |
| `p \| rpl::map(_1 > 0)` / `rpl::map(!_1)` | `p.map(\|v\| v > 0)` / `p.map(\|v: bool\| !v)` | `rpl::map(!rpl::mappers::_1)` ×22 |
| `p \| rpl::map_to(true)` / `p \| rpl::to_empty` | `p.map_to(true)` / `p.to_empty()` | `mainwidget.cpp:950` `_player->heightValue() \| rpl::map_to(true)` |
| `p \| rpl::map_error(f)` / `rpl::map_error_to_done()` | `p.map_error(f)` / `p.map_error_to_done()` | — |
| `p \| rpl::filter([=](const T &v) { ... })` | `p.filter(move \|v: &T\| ...)` | `session.cpp:181` `\| rpl::filter([=](DcId dcId) {` |
| `p \| rpl::filter(!_1)` / `rpl::filter(_1)` | `p.filter(\|v\| !*v)` / `p.filter(\|v\| *v)` | `mainwindow.cpp:144` `\| rpl::skip(1) \| rpl::filter(!rpl::mappers::_1)` → `.skip(1).filter(\|active\| !*active)` |
| `p \| rpl::filter(std::move(producerOfBool))` | `p.filter_producer(producer_of_bool)` | — |
| `p \| rpl::filter_optional()` / `filter_nullptr()` | `p.filter_optional()` / `p.filter_nullptr()` (оба для `Producer<Option<T>>`) | `data_replies_list.cpp:280` `_fullCount.value() \| rpl::filter_optional()` |
| `p \| rpl::distinct_until_changed()` | `p.distinct_until_changed()` | `api_reactions_notify_settings.cpp:125` |
| `p \| rpl::take(1)` / `skip(1)` / `take_while(f)` | `p.take(1)` / `p.skip(1)` / `p.take_while(\|v\| ...)` | `api_premium.cpp:833` `\| rpl::take(1) \| rpl::map(random)` |
| `a \| rpl::then(b)` | `a.then(b)` | `api_peer_colors.cpp:81` `\| rpl::then(_colorIndicesChanged.events() \| rpl::map([=] {...}))` |
| `rpl::deferred([=] { return x.value(); })` | `rpl::deferred(move \|\| x.value())` (замыкание `Clone`) | `media_stories_reply.cpp:183` |
| `rpl::combine(a, b)` | `rpl::combine((a, b))` → `Producer<(A, B)>` | 417 вызовов на 2 producer |
| `rpl::combine(a, b, _1 \|\| _2)` / `rpl::combine(a, b, [](A a, B b) {...})` | `rpl::combine((a, b)).map(\|(a, b)\| a \|\| b)` | `api_global_privacy.cpp:98` `rpl::combine(archiveAndMute(), _showArchiveAndMute.value(), _1 \|\| _2)` |
| `rpl::combine(std::move(vector))` / `(vector, mapper)` | `rpl::combine_vec(vector)` / `.map(mapper)` | `operators_tests.cpp` |
| `rpl::merge(a, b, c)` | `rpl::merge((a, b, c))` | `tray.cpp:178` `rpl::merge(_tray.showFromTrayRequests(), _minimizeMenuItemClicks.events() \| rpl::filter([=] {...}))` |
| `rpl::conditional(test, a, b)` | `rpl::conditional(test, a, b)` | `create_ai_box.cpp:553` |
| `p \| rpl::combine_previous()` / `combine_previous(v)` | `p.combine_previous()` / `p.combine_previous_with_default(v)` → `Producer<(T, T)>` | `tray.cpp:38` `workModeValue() \| rpl::combine_previous() \| rpl::on_next([=](WorkMode previous, WorkMode state) {` → `.on_next(move \|(previous, state)\| ...)` |
| `p \| rpl::flatten_latest()` | `p.flatten_latest()` | `tray.cpp:116` |
| `p \| rpl::before_next(f)` / `after_next(f)` | `p.before_next(\|v: &T\| ...)` / `p.after_next(\|v: T\| ...)` | `edit_todo_list_box.cpp:1036` `addedCount() \| rpl::after_next([=](int count) {` |
| `cond ? x.events() : (y.events() \| rpl::type_erased)` | `if cond { x.events() } else { y.events().type_erased() }` (тождество) | `api_blocked_peers.cpp:207` |
| `producer<T>()` (пустой) / `if (producer)` | `Producer::default()` / `!producer.is_empty()` | — |
| `rpl::no_value` / `rpl::no_error` / `rpl::empty_value` / `rpl::empty_error` / `rpl::empty` | `NoValue` / `NoError` / `()` / `EmptyError` / `EMPTY` (= `()`) | — |
| `rpl::details::...`, `superset_type`, `type_list`, `callable` | не нужны: роль играет система типов Rust | — |

## Модель

* **`Producer<V, E>`** — всегда со стёртым типом (как сборка оригинала в debug или под MSVC). Генератор — `FnOnce(Consumer) -> Lifetime + Clone`. При запуске генератор перемещается, поэтому `single(value)` отдаёт значение без копирования (это проверяют тесты оригинала со счётчиком копий). Клонирование producer (`duplicate`) копирует генератор со всем захваченным, как копия `std::function`; стоимость — одна аллокация на оператор цепочки.
* **`Consumer<V, E>`** — `Rc` на общие обработчики. Обработчик `next` — `Fn`: в C++ обработчик копируется перед каждым вызовом, так что `mutable`-лямбда изменений не сохраняет. Изменяемое состояние кладут в `Cell`/`RefCell` или в `make_state`. Обработчики `error` и `done` — `FnOnce`.
* **`Lifetime`** — список колбэков, вызываемых в обратном порядке при `destroy()` или `drop`. Все методы принимают `&self`: в оригинале lifetime почти всегда доступен по ссылке (`widget->lifetime()`), а колбэки могут пополнять или уничтожать тот самый lifetime, который их вызывает.
* **`make_state(value) -> StateRef<T>`.** Значение живёт ровно до `destroy()`, как `delete` в C++. Ручка слабая, поэтому её захват в обработчики, зарегистрированные в том же lifetime, не создаёт цикла. `get()` после уничтожения паникует (в C++ это висячий указатель), `try_get()` возвращает `None`. Удерживаемый `Rc` из `get()` продлевает жизнь состояния до конца обработчика.
* **`EventStream`** и **`Variable`** работают через `&self`. Ни один `RefCell` не заимствован, пока выполняется пользовательский код.
* **Тип ошибки фабрик.** `single`, `never`, `complete`, `vector` возвращают `NoError`, иначе вывод типов ломается в цепочках. Варианты с `_e` обобщены по ошибке.
* **`combine` и `merge`** принимают кортеж от 1 до 12 producer (в клиенте максимум — 10). У всех producer должен быть один тип ошибки: в C++ это вариант разных ошибок, но в клиенте везде `no_error`.

## Потокобезопасность

Оригинал однопоточный:

* в `event_stream.h` прямо сказано «Currently not thread-safe :(»;
* `rpl::lifetime` — обычный `std::vector`;
* флаг `_terminated` у consumer — обычный `bool`;
* атомарен только счётчик ссылок `std::shared_ptr`, и это свойство стандартной библиотеки C++, а не гарантия `lib_rpl`.

В клиенте каждый объект `rpl` создаётся, запускается, получает fire и уничтожается в одном потоке. Обычно это главный поток. Объекты mtproto и аудио живут в своём потоке целиком, подписки создаются уже в нём (`connection_tcp.cpp`, `media_audio.cpp`). Фоновые задачи возвращают результат через `crl::on_main(...)` и вызывают fire уже на главном потоке (`media_streaming_reader.cpp:1015`, `window_history_hider.cpp:85`).

Поэтому крейт построен на `Rc`/`Cell`/`RefCell`, и все типы `!Send` и `!Sync` (это проверяют `compile_fail`-доктесты в `lib.rs`). Гарантия та же, что в оригинале: использование из одного потока. Нарушения, которые в C++ были бы гонками данных, здесь становятся ошибками компиляции. `Arc` с `Mutex` обещали бы потокобезопасность, которой в оригинале нет, и замедлили бы доставку каждого значения.

Следствие, которое строже C++: объект с `Lifetime`/`EventStream`/`Variable` нельзя создать в одном потоке и переместить в другой (паттерн Qt `moveToThread`). Такие объекты создаются в том потоке, где живут. Для `tg-runtime` отсюда нужна операция «выполнить на главном потоке», принимающая не-`Send` замыкания, если её вызывают с главного потока.

## Реентерабельность

Повторяет оригинал. Всё перечисленное покрыто тестами `tests/reentrancy_tests.rs` и `tests/producer_tests.rs`.

* **`fire` изнутри обработчика того же потока.** Значение доставляется в глубину: вложенный `fire` доходит до всех consumer раньше, чем внешний продолжит. Пример: A получает 1 и вызывает fire(2) — порядок A1, A2, B2, B1.
* **Подписка во время `fire`.** Новый consumer не получает текущее значение, только следующие.
* **Отписка во время `fire`, в том числе самого себя.** Отписанный consumer сразу перестаёт получать значения. Из списка его удаляет самый внешний `fire`; до этого `has_consumers()` его ещё учитывает, как в оригинале. Обработчик, уничтоживший свой lifetime, продолжает выполняться, его `Rc` жив.
* **Уничтожение потока или `fire_done` из обработчика.** Идущий `fire` дорабатывает на своей копии списка. Оставшиеся consumer уже получили `done` и пропускаются. Новые `events()` после `fire_done` начинают новый список.
* **`Variable::set` из обработчика её же `value()`.** Работает. Пока доставляется начальное значение, consumer ещё не подписан на `changes`, как в оригинале (`single(...) | then(events())`), поэтому присваивание в этот момент он не увидит.
* **`combine_previous` при вложенной доставке.** Пара отправляется до замены «предыдущего» значения, как в C++.

### Дефект оригинала, перенесённый намеренно

`fire_forward` уплотняет вектор consumer через `std::remove_if` с индексными итераторами. Вложенный `fire` уплотняет тот же вектор, но не удаляет хвост. Если перед стреляющим consumer есть отписанный, но ещё не удалённый consumer, внешний `remove_if` после возврата записывает поверх живого consumer. Итог:

* живой consumer пропадает из списка, хотя его lifetime жив;
* последний consumer не получает внешнее значение;
* consumer, подписанный во время вложенного `fire`, может оказаться в списке дважды и получать каждое значение два раза.

Алгоритм перенесён буквально, вплоть до реализации `remove_if` из libstdc++. Поведение закреплено тестами `nested_fire_with_stale_consumer_loses_live_consumer_like_original` и `nested_fire_with_new_consumer_duplicates_it_and_loses_firing_one_like_original`.

## Отличия от оригинала

1. **Обработчики consumer освобождаются при terminate**, сразу после уничтожения его lifetime. В C++ они живут, пока не уйдёт последняя копия consumer. Захваченные значения освобождаются раньше и детерминированно, циклы через обработчики разрываются. Виден этот момент только деструкторам захваченных объектов (тест `termination_drops_handlers_and_their_captures`).
2. **`Lifetime::release()` освобождает состояние из `make_state`.** В C++ оно утекает, потому что колбэк `delete` просто забывается. В клиенте `release()` не вызывается.
3. **`consumer.make_state()` у уже завершённого consumer** возвращает мёртвую `StateRef` вместо `nullptr`. Операторы крейта в этом случае молча пропускают значение; C++ разыменовал бы `nullptr`.
4. **`start_to_stream` захватывает поток слабой ссылкой, а не ссылкой C++.** fire в уже уничтоженный поток ничего не делает, тогда как в C++ это висячая ссылка.
5. **`combine_previous` клонирует старое значение в пару.** C++ перемещает его, оставляя в состоянии moved-from объект на время обработчика. Порядок операций совпадает; разница только в том, что при вложенной доставке видно «предыдущее» значение вместо moved-from объекта. Для тривиально копируемых типов поведение идентично. Цена — один лишний `clone`.
6. **Перемещение `Variable` переносит подписчиков.** C++-перемещение переносит только значение. Если код на это опирается, пишите `b.set(a.current())`.
7. **`Variable` требует выбрать, есть ли сравнение:** `new` или `new_non_comparable`. В C++ это определяется на этапе компиляции по наличию `operator==`.
8. **`fire_done` обходит consumer по снимку длины списка.** В C++ range-for по вектору, растущему из обработчика `done`, — неопределённое поведение.
9. **`then`, `combine` и `merge` требуют одинаковых типов** значения и ошибки (в C++ — `superset_type`, вариант ошибок). В самой библиотеке определён только тождественный `superset_type`.
10. **`take` и `skip` принимают `usize`**, вместо `Expects(count >= 0)`.
11. **`filter_nullptr` работает на `Producer<Option<T>>`** и совпадает с `filter_optional`: нулевые указатели в Rust — это `Option`.
12. **Типы `!Send` и `!Sync`** (см. «Потокобезопасность»).

## Файлы оригинала

| Оригинал | Модуль | Итог |
|---|---|---|
| `rpl/lifetime.h` | `lifetime.rs` | переведён полностью |
| `rpl/consumer.h` | `consumer.rs` | переведён полностью; специализация без стирания типа не нужна |
| `rpl/producer.h` | `producer.rs` | переведён полностью; `on_*`/`start` стали методами |
| `rpl/event_stream.h` | `event_stream.rs` | переведён полностью, вместе с `start_to_stream` и `start_spawning` |
| `rpl/variable.h` | `variable.rs` | переведён полностью |
| `rpl/range.h` | `range.rs` | переведён полностью |
| `rpl/complete.h`, `fail.h`, `never.h`, `deferred.h` | одноимённые | переведены полностью |
| `rpl/map.h`, `filter.h`, `take.h`, `skip.h`, `then.h` | одноимённые | переведены полностью |
| `rpl/merge.h`, `combine.h`, `combine_previous.h`, `conditional.h` | одноимённые | переведены полностью |
| `rpl/distinct_until_changed.h`, `flatten_latest.h`, `type_erased.h`, `before_next.h`, `after_next.h` | одноимённые | переведены полностью |
| `rpl/rpl.h` | `lib.rs` | переведён полностью |
| `rpl/mappers.h` | — | заменён замыканиями |
| `rpl/details/callable.h`, `superset_type.h`, `type_list.h` | — | заменены системой типов Rust |
| `rpl/producer_tests.cpp`, `operators_tests.cpp`, `variable_tests.cpp` | `tests/*.rs` | перенесены все 28 секций |
| `CMakeLists.txt` | `Cargo.toml` | заменён |
