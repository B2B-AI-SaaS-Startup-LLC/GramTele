# tg-base: контейнеры и языковые утилиты

Порт части `Telegram/lib_base/base/` (оригинал: AyuGram Desktop 7.2.9, коммит `0f2a31b`). Документ для тех, кто переводит клиент файл за файлом: слева — как писали в C++, справа — как писать в Rust. Количества использований посчитаны grep-ом по `Telegram/SourceFiles` и `Telegram/lib_*`.

Решения трёх видов: **переведён** (в `tg-base` есть свой код), **заменён** (кода нет, роль выполняет std или язык), **отложен** (относится к другому слою). Отложенных файлов в этой группе нет.

## Сводная таблица решений

| Файл | Решение | Обоснование | Статус реестра |
|---|---|---|---|
| flat_map | переведён (`flat_map`, `FlatMap`, `FlatMultiMap`) | 987 использований в 378 файлах, нужны порядок, позиции, multi-вариант, свой компаратор | done |
| flat_set | переведён (`FlatSet`, `FlatMultiSet`) | 704 использования в 252 файлах, нужны `modify`, `merge`, позиции | done |
| flags | переведён (`Flags<E>`, `flag_type!`) | 95 использований `base::flags<>`, 68 `is_flag_type`; типобезопасные операторы | done |
| enum_mask | переведён (`EnumMask<E>`) | 4 использования (`SharedMediaTypesMask`), крошечный | done |
| ordered_set | заменён (`BTreeSet`) | 5 использований, только `contains`/`insert` | replaced |
| last_used_cache | переведён (`LastUsedCache`) | в клиенте не используется, тривиален, поведение сохранено | done |
| index_based_iterator | переведён (`IndexBasedIterator`) | нужен только тесту `algorithm_tests`; в клиенте итератор не нужен | done |
| algorithm | переведён частично-заменён | `reorder`, `reorder_index`, `accumulate_*`, `safe_round` и др. переведены, остальное в std | done |
| basic_types | заменён (+ `TimeId`) | алиасы целых/`Fn`/`not_null` покрывает язык; `TimeId` оставлен | done |
| const_string | заменён (`&str`) | `"..."_cs` в 7 файлах, это `string_view` | replaced |
| optional | заменён (`Option`) | `optional | func` цепочки = `Option::map` | replaced |
| variant | заменён (`enum`) | `std::variant`/`v::match` — нативный `enum` + `match` | replaced |
| expected | заменён (`Result`) | `base::expected` в 9 файлах | replaced |
| overload | заменён (`match`) | `base::overload` нужен был для visit | replaced |
| match_method | заменён (`match`) | 2 файла, тоже visit | replaced |
| functors | заменён (замыкания) | `func::abs/add/negate` в 9 местах | replaced |
| type_traits | заменён | `parameter_type`/`is_fast_copy_type` — выбор «по значению или по ссылке» делает программист | replaced |
| unique_any | заменён (`Box<dyn Any>`) | в клиенте не используется | replaced |
| unique_function | заменён (`Box<dyn FnMut>`) | ~380 использований через `FnMut<>`; move-only функция — норма Rust | replaced |
| virtual_method | заменён (трейты/`enum`) | не используется нигде, кроме CMake | replaced |
| required | заменён (обычное поле) | 9 использований; в Rust литерал структуры требует все поля | replaced |
| custom_delete | заменён (`Drop`) | используется только в `lib_base/platform/linux` для `unique_ptr<void, deleter>` | replaced |
| never_freed_pointer | переведён (`NeverFreedPointer`) | 4 глобала | done |
| atomic | заменён (`std::sync::atomic`) | `base::atomic` нигде не используется | replaced |
| thread_safe_wrap | переведён (`ThreadSafeWrap`, `ThreadSafeQueue`) | используется в media streaming reader | done |
| build_config | заменён (`cfg!`, `#[inline(always)]`) | макросы компилятора/архитектуры; используется в одном файле | replaced |
| assertion | переведён (`expects!`, `ensures!`, `tg_assert!`, `unexpected!`) | 2957 `Expects`, 1248 `Assert`, 731 `Unexpected` | done |
| debug_log | переведён (`log!`, `debug_log!`, `profile_log!`) | 1774 `LOG`, 445 `DEBUG_LOG` | done |
| weak_ptr | переведён с переосмыслением (`HasWeakPtr`, `WeakPtr<T>`, `Alive`) | 154 `has_weak_ptr`, 722 `make_weak`; под модель хранилищ | done |
| binary_guard | переведён (`BinaryGuard`) | 19 файлов | done |
| runtime_composer | переведён (`RuntimeComposer<Base>`) | основа `HistoryItem`, `Element`, `Document`, `AbstractLayoutItem`, ~60 компонентов | done |

`last_user_input`, `unique_qptr`, `weak_qptr`, `custom_app_icon` в эту группу не входят.

## flat_map / flat_multi_map

Клиент опирается на: порядок обхода по ключу; итераторы произвольного доступа (`begin() + n`, 236 мест); `lower_bound`/`upper_bound` (98/28); `erase(it)`, возвращающий следующий, и `erase(first, last)`; `flat_multi_map` с повторами (12 мест: `_pollsClosings`, `_expiring`, `_commandByObject`); `findFirst`, `remove(key, value)`; компараторы (76 объявлений с третьим параметром); `emplace_or_assign`, `try_emplace`. `BTreeMap` этого не даёт (нет позиций, нет множественных ключей), поэтому нужен свой тип.

Позиция итератора — `usize`: `end()` это `len()`, `begin() + n` это `n`. Пары отдаются как `(&K, &V)`, ключ изменять нельзя.

| C++ | Rust |
|---|---|
| `base::flat_map<K, V>` / `flat_map<K, V, std::greater<>>` | `FlatMap<K, V>` / `FlatMap<K, V, Greater>` |
| `base::flat_multi_map<TimeId, not_null<PollData*>>` | `FlatMultiMap<TimeId, PollId>` |
| `map.emplace(k, v)` (не перезаписывает) | `map.emplace(k, v)` → `(pos, inserted)` |
| `map.insert_or_assign(k, v)`, `emplace_or_assign` | `map.insert_or_assign(k, v)` |
| `map.try_emplace(k, args...)` | `map.try_emplace(k, v)` или ленивый `try_emplace_with(k, \|\| v)` |
| `map[k]` | `*map.get_or_default(k)` (нужен `V: Default`) |
| `const auto i = map.find(k); if (i != end(map)) use(i->second)` | `if let Some(v) = map.get(&k) { use(v) }` |
| `i = map.find(k)` (нужна позиция) | `map.find(&k)` → `Option<usize>` |
| `i->first`, `i->second` | `let (k, v) = map.get_at(i).unwrap()` или `for (k, v) in &map` |
| `for (auto &[k, v] : map) v = ...` | `for (k, v) in &mut map { *v = ... }` |
| `map.begin() + n` | `map.get_at(n)`, `map.as_slice()[n]` |
| `map.front()`, `map.back()` | `map.front()`, `map.back()` → `Option<(&K, &V)>` |
| `map.take(k)` | `map.take(&k)` → `Option<V>` |
| `map.remove(k)` / `map.erase(k)` | `map.remove(&k)` → `bool` / `map.erase(&k)` → `usize` |
| `i = map.erase(i)` в цикле | `i = map.erase_at(i)` (возвращает ту же позицию) |
| `map.erase(begin(map), i)` | `map.erase_range(0..i)` |
| `map.lower_bound(k)`, `upper_bound(k)` | то же, возвращают `usize` |
| `map.equal_range(k)` | `map.equal_range(&k)` → `Range<usize>` |
| `multi.findFirst(k)`, `removeOne`, `removeAll`, `remove(k, v)`, `count` | `find_first(&k)` → `Option<usize>`, `remove_one`, `remove_all`, `remove(&k, &v)`, `count` |
| `flat_multi_map_pair_type<K, V>(k, v)` (обходной приём в `format_values.cpp`) | обычный кортеж `(k, v)` |
| `base::flat_map<K, V>(begin(list), end(list))` | `list.into_iter().collect::<FlatMap<_, _>>()` |
| `std::less<>` с прозрачным поиском по `QStringView` | `String` как ключ, поиск `map.get("abc")` через `Borrow<str>` |

Пример из клиента (`data_session.cpp`, закрытие опросов): цикл с `erase`.

```cpp
for (auto i = _pollsClosings.begin(); i != _pollsClosings.end();) {
	if (i->first <= now) {
		... i = _pollsClosings.erase(i);
	} else { ...; ++i; }
}
```
```rust
let mut i = 0;
while i < self.polls_closings.len() {
    let (&time, &poll) = self.polls_closings.get_at(i).unwrap();
    if time <= now { ...; i = self.polls_closings.erase_at(i); } else { ...; i += 1; }
}
```

Пример `data_stories.cpp` (`processExpired`): `_expiring.erase(begin(_expiring), i)` — `let end = expiring.upper_bound(&now); expiring.erase_range(0..end);`.

Компараторы: трейт `Compare<T>` (`less(a, b)`), стандартные `Less` и `Greater`. Для прозрачного поиска (`is_transparent`, 4 места) нужно реализовать `Borrow<Q>` для ключа и `Compare<Q>` для компаратора; порядок по `Q` обязан совпадать с порядком по ключу.

Равные ключи в multi остаются в порядке вставки. При построении из диапазона с повторами в `FlatMap` побеждает первый (в C++ `std::sort` нестабилен, но тест оригинала ждёт именно первый).

## flat_set / flat_multi_set

Те же решения. `modify(it, action)` (10 мест, `data_messages.cpp`), `merge(first, last)` (15), `findFirst`, `removeOne`.

| C++ | Rust |
|---|---|
| `base::flat_set<MsgId>` | `FlatSet<MsgId>` |
| `set.insert(x)` / `set.emplace(x)` | `set.insert(x)` → `(pos, inserted)` |
| `set.contains(x)`, `set.find(x)` | `set.contains(&x)`, `set.find(&x)` → `Option<usize>` |
| `set.remove(x)`, `set.erase(x)` | `set.remove(&x)` → `bool`, `set.erase(&x)` → `usize` |
| `set.merge(a.begin(), a.end())` | `set.merge(a.iter().copied())` |
| `_slices.modify(it, [&](Slice &s) {...})` | `slices.modify(pos, \|s\| {...})` |
| `*set.begin()`, `*(set.end() - 1)` | `set.front()`, `set.back()` → `Option<&T>` |
| `for (const auto &x : set)` | `for x in &set` |

Известная особенность оригинала, перенесённая как есть: цикл в `modify` сдвигает элемент с фиксированной позицией `which`, поэтому порядок восстанавливается верно, только если элемент сместился не более чем на одну позицию (иначе контейнер остаётся несортированным; `[1,5,6]`, `modify(0, =10)` даёт `[5,10,6]`). Тест `modify_longer_move_is_unsorted_like_original` закрепляет это.

## flags

`base::flags<Enum>` + свободные операторы по `is_flag_type`. Перенос механический:

| C++ | Rust |
|---|---|
| `enum class Error : uchar {...}; friend constexpr inline bool is_flag_type(Error) { return true; }` | `#[derive(Clone, Copy, PartialEq, Eq)] #[repr(u8)] enum Error {...}` + `tg_base::flag_type!(Error: u8);` |
| `using Errors = base::flags<Error>;` | `type Errors = Flags<Error>;` |
| `base::flags<MessageFlag> flags = MessageFlag::AdminLogEntry;` | `let flags: Flags<MessageFlag> = MessageFlag::AdminLogEntry.into();` |
| `a \| b`, `a & ~b`, `a ^ b` | то же; результат `Flags<E>` |
| `flags \|= X`, `flags &= ~X`, `flags ^= X` | то же |
| `if (flags & Flag::X)` | `if (flags & Flag::X).any()` |
| `if (!(flags & Flag::X))` | `if (flags & Flag::X).is_empty()` |
| `flags == Flag::X`, `flags != ...` | то же (`PartialEq<E>`), сравнения `<`, `>` — по числу |
| `flags = 0`, `flags = nullptr`, `Flag \| 0` | `Flags::empty()` / `Flags::default()` / `Flags::from(Flag)` |
| `flags.value()`, неявное приведение к `Type` | `flags.value()` |
| `Flags::from_raw(x)` | `Flags::from_raw(x)` |
| TL-флаги `MTPDmessage::Flags` | генератор TL выдаёт `flag_type!(Flag: u32)` и `type Flags = Flags<Flag>` |
| `extended_flags<Ext>::type = Base` | `tg_base::extended_flags!(Ext => Base);` |

Нет неявных приведений к `bool` и числу: это единственное место, где перевод не дословный (`any`, `is_empty`, `value`). Операторы `|` для enum-значений возвращают `Flags<E>`, поэтому `Flag::A | Flag::B` можно сразу присвоить. `!flag` из C++ (логическое отрицание) → `.is_empty()` на значении `Flags`; `~flag` → `!flag`. Тип `#[repr]` обязан совпадать с типом в `flag_type!`.

## enum_mask

`base::enum_mask<SharedMediaType>` → `EnumMask<SharedMediaType>`; для enum нужно `impl EnumMaskValue` (`COUNT` = `kCount`, `index()`). `mask.test(x)`, `mask.set(other)`, `mask.added(other)`, `EnumMask::all()`, `if (mask)` → `mask.any()`.

## ordered_set

`OrderedSet<DocumentId> read; if (read.contains(id)) continue; read.insert(id);` (`storage_account.cpp`) → `BTreeSet<DocumentId>` (`insert` возвращает `bool`, пригодится вместо пары `contains`/`insert`). Остальные: `OrderedSet<ushort>` (`lang_instance.cpp`), `OrderedSet<QLatin1String>` (`window_theme_editor.cpp`), `OrderedSet<AbstractStructure**>` (`data_abstract_structure.cpp`; там нужен `BTreeSet` от идентификатора, не от указателя).

## last_used_cache, index_based_iterator

`last_used_cache<Entry>` → `LastUsedCache<Entry>` (`up`, `remove`, `clear`, `take_lowest`; пустой кеш возвращает `Entry::default()`, как оригинал). Внутри — счётчик «тиков» вместо списка итераторов, сложность `O(log n)`.

`index_based_iterator` нужен только чтобы переживать `push_back` в предикате `remove_if`. Заимствующий итератор в Rust невозможен, поэтому курсор хранит индекс и идентичность контейнера, а доступ идёт через `cursor.get(&vec)`. В коде клиента его использовать не придётся: пиши цикл по индексам.

## algorithm

| C++ | Rust |
|---|---|
| `base::take(x)` (1241 место) | `std::mem::take(&mut x)` |
| `base::duplicate(x)` (93) | `x.clone()` |
| `base::array_size(a)` | `a.len()` |
| `base::contains(c, v)` (22) | `c.contains(&v)` / `c.iter().any(...)` |
| `base::reorder(c, old, new)` (227) | `algorithm::reorder(&mut c, old, new)` |
| `base::reorder_index(i, old, new)` | `algorithm::reorder_index(i, old, new)` |
| `accumulate_max(a, b)`, `accumulate_min` (178/75) | то же, `accumulate_max(&mut a, b)` |
| `base::SafeRound(x)` (507) | `algorithm::safe_round(x)` (NaN → паника через `expects!`) |
| `base::OppositeSigns(a, b)` | `algorithm::opposite_signs(a, b)` |
| `base::CleanAndSimplify(s)` | `algorithm::clean_and_simplify(&s)` |
| `base::FromUtf8Safe(bytes)` | `algorithm::from_utf8_safe(&bytes)` |
| `base::up_cast<D>(p)` | `From`/`Into` или приведение к `&dyn Trait` |
| `qstr("..")` | `"..."` |
| `base::pointer_comparator<T>` (`calls_group_call.h`) | ключ по `Rc::as_ptr` / идентификатору |

`FromUtf8Safe` в оригинале при невалидном UTF-8 вызывает `fromLocal8Bit` (на Windows — ANSI-страница); в Rust невалидные байты становятся `U+FFFD`.

## basic_types, type_traits, functors, const_string

| C++ | Rust |
|---|---|
| `int8/uint8/.../int64/uint64`, `float32/64` | `i8/u8/.../i64/u64`, `f32/f64` |
| `TimeId` | `tg_base::basic_types::TimeId` (`i32`) |
| `not_null<T*>` | `&T`/`&mut T` или типизированный идентификатор |
| `index_type`, `gsl::index` | `isize`/`usize` |
| `Fn<void(int)>` | `Rc<dyn Fn(i32)>` (между потоками `Arc<dyn Fn(i32) + Send + Sync>`) |
| `FnMut<void()>` (`unique_function`) | `Box<dyn FnMut()>`; пустая — `Option<Box<dyn FnMut()>>` |
| `"x"_q`, `"x"_cs` | `"x"` |
| `base::parameter_type<T>` | `T` или `&T` по месту; `custom_is_fast_copy_type` (1 место) удалить |
| `result \| func::negate \| func::add(size() - 1)` | `result.map(\|v\| -v + size - 1)` |
| `x \| func::abs` | `x.map(i32::abs)` |

## optional, variant, expected, overload, match_method

| C++ | Rust |
|---|---|
| `std::optional<T>`, `std::nullopt` | `Option<T>`, `None` |
| `opt \| [](auto &v) {...}` | `opt.map(\|v\| ...)` |
| `std::variant<A, B>` | `enum X { A(A), B(B) }` |
| `v::null_t` (`mainwindow.h`) | пустой вариант `Null` или `Option` |
| `v::match(origin.data, [&](A a){...}, [&](B b){...})` (151 место) | `match origin.data { Data::A(a) => ..., Data::B(b) => ... }` |
| `[](auto&&) {...}` как «остальное» | `_ => ...` |
| `v::is<A>(x)`, `v::get<A>(x)` | `matches!(x, X::A(_))`, `if let X::A(a) = &x` |
| `base::overload(...)` (`image_location.cpp`) | `match` или набор замыканий |
| `base::match_method(...)` | `match` |
| `base::expected<not_null<Story*>, NoStory>` | `Result<StoryId, NoStory>`, `unexpected(e)` → `Err(e)` |

## unique_any, virtual_method, required, custom_delete, atomic, build_config

- `base::unique_any` — не используется. `Box<dyn Any>`.
- `virtual_method`/`virtual_object` — не используется нигде. Мультиметоды → трейт или `match` по паре.
- `base::required<T>` (`calendar_box.h`, `single_choice_box.h`): `struct Args { required<QDate> month; ... }` с designated-инициализацией → в Rust поля структуры и так обязательны; если рядом есть необязательные, использовать `Args::new(required...)` и `Default` для остального.
- `custom_delete<dlclose>` → `Drop` в обёртке FFI.
- `base::atomic` (с `wait`/`atomic_notify_all`) — не используется. `AtomicX`; ожидание — `Condvar`.
- `build_config.h` (`ARCH_CPU_64_BITS`, `TG_FORCE_INLINE`) → `cfg!(target_pointer_width = "64")`, `#[inline(always)]`.

## never_freed_pointer, thread_safe_wrap

| C++ | Rust |
|---|---|
| `base::NeverFreedPointer<T> structures; structures.createIfNull();` (`data_abstract_structure.cpp`, `window_theme.cpp`) | `NeverFreedPointer<T>`; глобал `thread_local!`/`static Mutex<...>`; `create_if_null(\|\| T::new())` |
| `structures->`, `*structures` | `Deref` (с проверкой `tg_assert!`) или `data()` → `Option<&T>` |
| `structures.clear()`, `isNull()`, `release()` | `clear()`, `is_null()`, `release()` → `Option<Box<T>>` |
| `base::thread_safe_wrap<T>::with(cb)` | `ThreadSafeWrap<T>::with(\|v\| ...)` |
| `base::thread_safe_queue<T>` (`media_streaming_reader.h`) | `ThreadSafeQueue<T>`; `take()` отдаёт `VecDeque<T>` вместо переданного `Container` |

## assertion, debug_log

| C++ | Rust |
|---|---|
| `Expects(cond)` (2957) | `expects!(cond)` |
| `Ensures(cond)` (92) | `ensures!(cond)` |
| `Assert(cond)` (1248) | `tg_assert!(cond)` |
| `Unexpected("Type in requestTermsUpdate().")` (731) | `unexpected!("Type in requestTermsUpdate().")`; тип `!` |
| `LOG(("Could not rename '%1' to '%2': %3").arg(a, b, c))` (1774) | `log!("Could not rename '{}' to '{}': {}", a, b, c)` |
| `DEBUG_LOG((..))` (445) | `debug_log!(..)` |
| `PROFILE_LOG((..))` | `profile_log!(..)` |
| `Integration::Instance().logMessage` | `debug_log::set_integration(Arc<dyn Integration>)` |

Проверки включены всегда, как в оригинале. При нарушении вызывается `Integration::log_assertion_violation` и `panic!` (в оригинале — запись по нулевому указателю ради краш-репорта). `%N` в строках перевода разворачивать в `{}` в порядке `.arg`. Трейт `Integration` определяет только логирующую часть `base::Integration`; полный `Integration` из `integration.cpp` должен реализовать его.

## weak_ptr / has_weak_ptr / make_weak

В C++ `base::weak_ptr<T>` — потокобезопасный слабый указатель; `weak.get()` возвращает `T*`. На нём строится `crl::guard`. В Rust, где объекты лежат в хранилищах с типизированными идентификаторами, указатель на объект не выдаётся, остаётся то, что реально нужно клиенту, — **живость**.

- `Alive` — трейт `fn alive(&self) -> bool`, на который опирается `tg-runtime` (`crl::guard`). Реализован для `WeakPtr<T>`, `BinaryGuard`, `std::rc::Weak`, `std::sync::Weak`.
- `HasWeakPtr` — встраиваемое поле (вместо базового класса). Умирает вместе с владельцем (`Drop`); `invalidate_weak_ptrs()` делает все выданные указатели мёртвыми, новые снова живы. Копирование владельца даёт свежее независимое состояние, как в оригинале.
- `WeakPtr<T>` — `Send + Sync`, дешёвый `Clone`, `Eq`/`Ord`/`Hash` по идентичности флага (можно ключом: `_draftsSaveRequestIds.emplace(base::make_weak(thread), 0)`).
- `AsHasWeakPtr` — трейт «у меня есть поле `HasWeakPtr`»; `make_weak(&obj)`.

| C++ | Rust |
|---|---|
| `class X : public base::has_weak_ptr` | поле `weak: HasWeakPtr` + `impl AsHasWeakPtr for X` |
| `const auto weak = base::make_weak(this);` (722) | `let weak = make_weak(self);` |
| `base::weak_ptr<Data::Thread> _w;` | `WeakPtr<Thread>` |
| `if (const auto strong = weak.get()) {...}` | `if weak.alive() { let obj = store.get(id)?; ... }` — доступ к объекту через хранилище по идентификатору |
| `weak.get() != nullptr`, `if (weak)` | `weak.alive()` / `!weak.is_empty()` |
| `invalidate_weak_ptrs(&_guard)` (в `calls_group_call.cpp`, `media_stories_controller.cpp` и др.) | `self.guard.invalidate_weak_ptrs()`, `guard: HasWeakPtr` |
| `weak_ptrs_count(&o)` | `o.weak_ptrs_count()` |
| `base::weak_ptr<Derived>` → `weak_ptr<Base>` | `weak.cast::<Base>()` |
| `crl::on_main(this, [=]{ ... })` (189 мест) | `tg_runtime::on_main(make_weak(self), move \|\| ...)`, где аргумент реализует `Alive`; доступ к `self` — через хранилище |

Ограничения: `WeakPtr` не доказывает, что объект всё ещё лежит в хранилище, — хранилище при удалении обязано уничтожить поле `HasWeakPtr` объекта (или вызвать `invalidate_weak_ptrs`). Разыменования нет сознательно. Объекты под `Rc`/`Arc` используют `Weak` из std. `weak == nullptr` в C++ сравнивает `get()`, а сравнение двух указателей — флаг; в Rust это `is_empty()` и `==`. Clippy на ключе `WeakPtr` в `HashMap` выдаёт `mutable_key_type`: разрешать в месте использования.

## binary_guard

`base::binary_guard` (19 файлов): `BinaryGuard`. `guard.make_guard()` возвращает партнёра и разрывает прежнюю связь; `guard.alive()`; `guard = nullptr` → `guard.reset()`. Живость обоих — один `Arc<AtomicBool>`. Реализует `Alive`.

## runtime_composer

`RuntimeComposer<Base>` — основа `HistoryItem`, `HistoryView::Element`, `HistoryView::Document`, `AbstractLayoutItem` (около 60 компонентов: `HistoryMessageEdited`, `HistoryMessageReply`, ...). Решение: типизированный по `TypeId` набор компонентов; битовые маски и лимит 64 пропали.

| C++ | Rust |
|---|---|
| `struct HistoryMessageEdited : RuntimeComponent<HistoryMessageEdited, HistoryItem>` | `#[derive(Default)] struct HistoryMessageEdited {...}` + `runtime_component!(HistoryMessageEdited, HistoryItem);` |
| `class HistoryItem : public RuntimeComposer<HistoryItem>` | поле `composer: RuntimeComposer<HistoryItem>` |
| `item->Has<HistoryMessageForwarded>()` | `item.composer.has::<HistoryMessageForwarded>()` |
| `item->Get<HistoryMessageEdited>()` | `item.composer.get::<HistoryMessageEdited>()` → `Option<&T>` |
| `AddComponents(HistoryServiceData::Bit()); Get<..>()->x = 1;` | `composer.get_or_add::<HistoryServiceData>().x = 1;` |
| `AddComponents(A::Bit() \| B::Bit())` | `composer.add::<A>(); composer.add::<B>();` |
| `RemoveComponents(HistoryMessageReplyMarkup::Bit())` | `composer.remove::<HistoryMessageReplyMarkup>()` |
| `RuntimeComposer(mask)` в конструкторе | цепочка `add::<T>()` после создания |

Компоненты создаются `Default`. Повторное `add` не затирает значение; `remove` + `add` даёт значение по умолчанию. Композер не `Send` (объекты главного потока).

## Отличия от поведения оригинала (сводка)

- `FlatMap::from(..)` с повторами берёт первый элемент; в C++ это зависело от нестабильного `std::sort`.
- Сортировка в Rust стабильная; компаратор, нарушающий строгий слабый порядок, может вызвать панику в `sort_by`.
- Цикл `modify` перенесён без исправлений (см. выше).
- `LogWriteDebug` в оригинале печатает `__LINE__` из `debug_log.cpp` вместо номера строки вызова: перенесено (всегда `25`).
- `unexpected!`/`expects!` паникуют, а не пишут по нулевому адресу; сообщение и вызов лога те же.
- `safe_round` не воспроизводит повторные попытки вокруг `fetestexcept` (в Rust `round` не-NaN значения не даёт NaN).
- `take_lowest()` пустого `LastUsedCache` возвращает `Entry::default()`, как в оригинале.
- Позиции вместо итераторов: код, хранивший итератор через вставки, должен хранить ключ.
