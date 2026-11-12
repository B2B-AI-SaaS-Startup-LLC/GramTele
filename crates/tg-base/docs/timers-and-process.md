# Таймеры и служебная часть tg-base

Исходник: AyuGram Desktop `0f2a31bb420de98f331990432265b131b8d836b5`.
Пути таблиц относительны `Telegram/lib_base/base/`. Это граница этапа 1,
а не обещание готового оконного/системного backend. mac/win не изменялись.
Статус «отложен» остаётся `todo` в ledger; номер ниже — этап возврата из PLAN.md,
а исходный столбец stage=1 в ledger сохраняет принадлежность lib_base.

## Устройство и проверенные контракты

TimerHost хранит расписание по идентификаторам, а не граф QObject. Timer/DelayedCallTimer
держат состояние отдельных handles; удаление удаляет слот, callback копируется перед вызовом.
SystemClock использует tg_runtime::now. Clock + Alarm абстрактны: ManualClock/ManualAlarm
позволяют проверять сроки без sleep. Alarm только сообщает минимальный deadline, владелец
вызывает process на своей очереди. init_main_timers связывает ThreadAlarm с главной очередью;
приложение должно установить wakeup main queue и вызывать process_main_queue.

callOnce/callEach, remainingTime, cancel/restart/drop из собственного обработчика,
порядок deadline/старта, одновременное удаление другого таймера, нулевой интервал,
DelayedCallTimer и producers timer_rpl проверены виртуальным временем.
Повторный Timer держит сетку Qt и пропускает пропущенные периоды, но remaining_time
оценивает `_next = now + timeout`, как C++. Adjust отменяет старый таймер и ставит
его на max(_next-now,0); после ближайшего вызова повторение начинается полным интервалом.
CheckLocalTime вызывает tg_runtime::adjust_time, сбрасывает HTTP unixtime и уведомляет
оба расписания. Интеграционный тест инъецирует решение о скачке, а не меняет системные часы.

ConcurrentTimerEnvironment — отдельный поток/расписание, доставка только через runner
очереди владельца tg-runtime (есть тест с настоящей Queue и виртуальным Clock).
Сохранены две ошибки C++: callEach срабатывает один раз; cancel обнуляет guard до isActive
и не посылает cancel environment. Guard всё равно подавляет callback. Это проверено тестами.
В клиенте streaming/cache/round recorder используют callOnce.

Alive теперь alias GuardToken, а WeakPtr, HasWeakPtr и BinaryGuard реализуют AsGuard.
BinaryGuardToken не владеет парой и не убивает её при drop клона. profile_log!
использует tg_runtime::profile, второго начала отсчёта нет.

options: bool/int32/String, JSON только отличий от defaults, UTF-16 порядок ключей,
отступ 4 пробела и финальный LF, compact JSON, удаление файла при сбросе, запись через
1000 мс. Образец tests/fixtures/experimental_options.json выписан по Qt QJsonWriter 6.8;
нативный Qt в этой среде не запускался. Импорт валидирует всё до изменения и сбрасывает
неуказанные настройки. Read сохраняет lenient toInt(default=0) Qt для дробных/непомещающихся чисел.
Registry явный, его нужно держать до завершения приложения и вызвать finish/flush при выходе.

single_instance: имя — padded base64url двух CRC32 от UTF-16LE application/path;
relative socket name разрешается относительно temp dir как QLocalServer на Linux.
Проверен независимый фиксированный образец имени и base64 wire `HELLO`/`PID:0;WND:0;`,
включая фрагментацию пакета и отдельный процесс. Lock — готовый file_lock, путь
`{data_path}_single_instance.tmp`; data_path должен иметь тот же завершающий slash,
что и в C++. Команды — произвольные bytes, завершаются `;` после base64url.
process/poll_reply надо вызывать по socket readiness из будущего loop.
Активация окна по PID/WId ещё не перенесена. Сохранён исходный CleanName относительно
cwd: stale socket в другом каталоге может сорвать bind. Совместимость проверена
образцами и std UnixStream процессом, обмен с исполняемым Qt-клиентом не запускался.

CrashReportHeader — только owned форматирование, лимит и числовой вывод, не handler.
Его нельзя вызывать из fatal signal. Безопасный high-level signal-hook не принимает
SIGSEGV/SIGILL/SIGFPE; low_level::register требует unsafe. Поэтому writer отложен до 6:
варианты — внешний Crashpad/Breakpad в FFI-крейте или отдельный crash helper с безопасным
API и предварительно выделенным буфером. Зависимость signal-hook не добавлена.

## Как писали в C++ → как писать в Rust

Все client paths ниже относительны Telegram/SourceFiles. Сетевой клиент и UI ещё не портированы;
это соответствия API, без фиктивного dispatcher или окна.

| C++ / пример из клиента | Rust |
|---|---|
| `base::Timer([=]{...})` (`core/application.cpp`, таймеры приложения) | `Timer::new(|| {...})` либо `Timer::in_host(&host, Some(callback))` |
| `_timer.callOnce(delay)` (`data/data_session.cpp`) | `timer.call_once(delay)` |
| `_timer.callEach(delay)` (application periodic work) | `timer.call_each(delay)` |
| `_timer.cancel()` | `timer.cancel()`; допустимо из callback |
| `_timer.remainingTime()` | `timer.remaining_time()`; -1 если inactive |
| `base::Timer::Adjust()` | `Timer::adjust_all()` и `ConcurrentTimerEnvironment::adjust_all()` через `check_local_time()` |
| `base::DelayedCallTimer::call(delay, callback)` | `delayed.call(delay, callback)`; возвращает id для cancel |
| `base::call_delayed(delay, guard, callback)` (`core/application.cpp`) | `call_delayed::call_delayed_guarded(delay, &guard, callback)` |
| `base::timer_once(delay)` | `timer_rpl::timer_once(delay)`; lifetime отменяет Timer |
| `base::timer_each(delay)` | `timer_rpl::timer_each(delay)` |
| `base::InvokeQueued(context, callback)` | `invoke_queued::invoke_queued(&guard, callback)` |
| `base::SingleQueuedInvokation` | `SingleQueuedInvokation` / `SingleQueuedInvokationLocal` |
| `base::ConcurrentTimer(owner)` (`media/streaming`, cache) | `ConcurrentTimer::new(runner, callback)` / `from_weak` |
| `base::binary_guard` как guard crl | `BinaryGuard` реализует `tg_runtime::AsGuard` |
| `base::weak_ptr` как guard crl | `WeakPtr` реализует `tg_runtime::AsGuard` |
| `PROFILE_LOG(...)` | `profile_log!(...)`, время из tg_runtime |
| `base::options::Option<bool>` (`core/application.cpp`) | `registry.register(Descriptor::<T>::new(id))`; handle.set/value/changes |
| `base::options::Init(path)` (`core/launcher.cpp`) | `registry.init(path)`; TimerHost уже установлен |
| `base::SingleInstance` (launcher layer) | `SingleInstance::start(app, path)?`; commands/process/reply |
| `base::BatterySaving` (`core/application.cpp`) | `BatterySaving::new(factory)`; factory обязана дать platform helper |
| `base::ScreenReaderState::Instance()` | `screen_reader_state::set_instance(...)`, затем `instance()` |
| `base::LastUserInputTime()` (activity/online state) | `last_user_input::last_user_input_time()` |
| `base::PowerSaveBlocker` (calls/video playback) | Сначала `set_backend`, затем `PowerSaveBlocker::new(...)` |

## Решение по каждому файлу

### Общие файлы

| Файл | Решение | Этап возврата | Реализация / что требуется |
|---|---|---:|---|
| `base_pch.h` | заменён | — | Rust use + Cargo modules, предварительно компилируемый C++ заголовок не нужен. |
| `battery_saving.cpp` | частично | 6 | Generic helper, Option<bool> и поток значений проверены; Linux GPowerProfileMonitor/GLib требует этап 6. Фабрика backend явная. |
| `battery_saving.h` | частично | 6 | Generic helper, Option<bool> и поток значений проверены; Linux GPowerProfileMonitor/GLib требует этап 6. Фабрика backend явная. |
| `binary_guard.h` | переведён | — | AsGuard/GuardToken из tg-runtime, guards проверены; profile_log использует tg_runtime::profile. |
| `call_delayed.cpp` | переведён | — | Одноимённый модуль; виртуальное время, отмена и жизненный цикл проверены тестами. |
| `call_delayed.h` | переведён | — | Одноимённый модуль; виртуальное время, отмена и жизненный цикл проверены тестами. |
| `concurrent_timer.cpp` | переведён | — | Одноимённый модуль; виртуальное время, отмена и жизненный цикл проверены тестами. |
| `concurrent_timer.h` | переведён | — | Одноимённый модуль; виртуальное время, отмена и жизненный цикл проверены тестами. |
| `crash_report_header.cpp` | частично | 6 | Проверен owned-форматтер 64 КиБ; глобальный буфер и async-signal-safe публикация требуют аварийной подсистемы. |
| `crash_report_header.h` | частично | 6 | Проверен owned-форматтер 64 КиБ; глобальный буфер и async-signal-safe публикация требуют аварийной подсистемы. |
| `crash_report_writer.cpp` | отложен | 6 | Отложен: SIGSEGV/backtrace/ucontext, аварийный fd, аннотации, внешняя crash-подсистема; unsafe запрещён. |
| `crash_report_writer.h` | отложен | 6 | Отложен: SIGSEGV/backtrace/ucontext, аварийный fd, аннотации, внешняя crash-подсистема; unsafe запрещён. |
| `custom_app_icon.h` | частично | 5 | Linux path/digest/clear None/false проверены; overload QImage ждёт tg-gfx этапа 5, mac — этап 10. |
| `debug_destroy_informer.h` | переведён | — | debug-only PrintDead<T>, Drop и Deref; тест уничтожения. Имя типа Rust вместо C++. |
| `debug_log.cpp` | переведён | — | AsGuard/GuardToken из tg-runtime, guards проверены; profile_log использует tg_runtime::profile. |
| `debug_log.h` | переведён | — | AsGuard/GuardToken из tg-runtime, guards проверены; profile_log использует tg_runtime::profile. |
| `event_filter.cpp` | отложен | 5 | Отложен: Доставка/отмена событий и parent lifetime: tg-window dispatcher; используется виджетами и LastUserInput. |
| `event_filter.h` | отложен | 5 | Отложен: Доставка/отмена событий и parent lifetime: tg-window dispatcher; используется виджетами и LastUserInput. |
| `global_shortcuts.h` | отложен | 6 | Отложен: native key descriptors/names, запись сочетаний и источник системных событий; клиент calls, запись PTT; tg-window этап 5, X11 backend этап 6. |
| `global_shortcuts_generic.cpp` | отложен | 6 | Отложен: native key descriptors/names, запись сочетаний и источник системных событий; клиент calls, запись PTT; tg-window этап 5, X11 backend этап 6. |
| `global_shortcuts_generic.h` | отложен | 6 | Отложен: native key descriptors/names, запись сочетаний и источник системных событий; клиент calls, запись PTT; tg-window этап 5, X11 backend этап 6. |
| `integration.cpp` | частично | 5 | Logging singleton и ExecutableInfo/std::env проверены; enterFromEventLoop и crash annotations требуют launcher/tg-window (5/6). |
| `integration.h` | частично | 5 | Logging singleton и ExecutableInfo/std::env проверены; enterFromEventLoop и crash annotations требуют launcher/tg-window (5/6). |
| `invoke_queued.h` | переведён | — | Одноимённый модуль; виртуальное время, отмена и жизненный цикл проверены тестами. |
| `last_user_input.cpp` | частично | 5 | Приоритет provider и отметка ввода проверены; tg-window event filter (5), XCB/Mutter (6) отложены. |
| `last_user_input.h` | частично | 5 | Приоритет provider и отметка ввода проверены; tg-window event filter (5), XCB/Mutter (6) отложены. |
| `network_reachability.cpp` | частично | 6 | Общий приоритет/поток доступности проверен; обязательный источник заменяет отсутствующий QNetworkInformation backend (этап 6). |
| `network_reachability.h` | частично | 6 | Общий приоритет/поток доступности проверен; обязательный источник заменяет отсутствующий QNetworkInformation backend (этап 6). |
| `object_ptr.h` | отложен | 5 | Отложен: Родитель QObject, QPointer обнуление, hide/deleteLater: хранилище виджетов tg-window; Box не сохраняет контракт. |
| `options.cpp` | переведён | — | Явный Registry вместо статической регистрации; Qt JSON, задержка 1000 мс, импорт и сброс проверены образцом. |
| `options.h` | переведён | — | Явный Registry вместо статической регистрации; Qt JSON, задержка 1000 мс, импорт и сброс проверены образцом. |
| `power_save_blocker.cpp` | частично | 6 | Группы блокировок, перевыбор окна и backend trait проверены fake backend; portal Inhibit/XCB требует этап 6. |
| `power_save_blocker.h` | частично | 6 | Группы блокировок, перевыбор окна и backend trait проверены fake backend; portal Inhibit/XCB требует этап 6. |
| `qt_connection.h` | отложен | 5 | Отложен: QObject connect/disconnect, queued delivery и уничтожение sender/receiver: tg-window. |
| `qt_signal_producer.h` | отложен | 5 | Отложен: Qt signal-to-rpl bridge с enterFromEventLoop: tg-window + tg-rpl, одного EventStream недостаточно. |
| `screen_reader_state.cpp` | частично | 5 | Generic observer/Variable проверен; QAccessible adapter (5) и Linux AT-SPI D-Bus (6) отложены. |
| `screen_reader_state.h` | частично | 5 | Generic observer/Variable проверен; QAccessible adapter (5) и Linux AT-SPI D-Bus (6) отложены. |
| `single_instance.cpp` | частично | 6 | Linux UnixStream/FileLock, Qt-имя и обмен проверены; автоматическая readiness-доставка и активация PID/WId требуют tg-window. |
| `single_instance.h` | частично | 6 | Linux UnixStream/FileLock, Qt-имя и обмен проверены; автоматическая readiness-доставка и активация PID/WId требуют tg-window. |
| `system_unlock.h` | переведён | — | Типы и настоящий Linux known=true/available=false/Cancelled проверены; mac/win не входят в эту работу. |
| `tests_main.cpp` | заменён | — | cargo test/libtest; вместо Catch entrypoint; subprocess tests сохраняют процессную изоляцию. |
| `timer.cpp` | переведён | — | Одноимённый модуль; виртуальное время, отмена и жизненный цикл проверены тестами. |
| `timer.h` | переведён | — | Одноимённый модуль; виртуальное время, отмена и жизненный цикл проверены тестами. |
| `timer_rpl.h` | переведён | — | Одноимённый модуль; виртуальное время, отмена и жизненный цикл проверены тестами. |
| `unique_qptr.h` | отложен | 5 | Отложен: QPointer плюс владение/очистка QObject: хранилище tg-window. |
| `weak_ptr.h` | переведён | — | AsGuard/GuardToken из tg-runtime, guards проверены; profile_log использует tg_runtime::profile. |
| `weak_qptr.h` | отложен | 5 | Отложен: Автоматическое обнуление и const observer QObject: tg-window IDs/generations. |

### Qt adapters

| Файл | Решение | Этап возврата | Реализация / что требуется |
|---|---|---:|---|
| `qt/qt_common_adapters.h` | отложен | 5 | Отложен: TouchDevice зависит от модели устройств ввода tg-window. |
| `qt/qt_compare.h` | отложен | 4 | Отложен: Ord/PartialOrd покрывают Vec/enum; порядок QString — UTF-16, не String::cmp. Отложено до миграции текстовых call sites этапа 4/5. |
| `qt/qt_key_modifiers.h` | отложен | 5 | Отложен: Глобальное состояние клавиатуры tg-window; сравнение равно одному модификатору, а не contains. |
| `qt/qt_string_view.h` | отложен | 4 | Отложен: StringViewMid принимает отрицательные индексы, UTF-16 и null/empty; нужен текстовый adapter, этап 4/5. |
| `qt/qt_tab_key.cpp` | отложен | 5 | Отложен: Цепочка фокуса, Tab/Backtab и фильтр событий tg-window. |
| `qt/qt_tab_key.h` | отложен | 5 | Отложен: Цепочка фокуса, Tab/Backtab и фильтр событий tg-window. |

### platform/ — общие файлы

| Файл | Решение | Этап возврата | Реализация / что требуется |
|---|---|---:|---|
| `platform/base_platform_custom_app_icon.h` | частично | 5 | Linux path/digest/clear проверены; overload QImage ждёт tg-gfx этапа 5. |
| `platform/base_platform_file_utilities.cpp` | заменён | — | Только include заголовка; модуль Rust не требует отдельной translation unit. |
| `platform/base_platform_file_utilities.h` | частично | 6 | Linux операции fs и current executable проверены; ShowInFolder + ресурсы требуют desktop backend. |
| `platform/base_platform_global_shortcuts.h` | отложен | 6 | Отложен: глобальные события, native descriptors и их display names. |
| `platform/base_platform_haptic.h` | частично | 6 | IsSwipeBackEnabled=true как на Linux; Haptic требует feedbackd D-Bus. |
| `platform/base_platform_info.cpp` | отложен | 4 | Отложен: Qt Unicode CleanAndSimplify, UTF-16 длина и device model; нужен текстовый adapter для parity. |
| `platform/base_platform_info.h` | отложен | 6 | Отложен: сведения ОС/сессии/локали, X11/Wayland, версия glibc; client launcher и device model. |
| `platform/base_platform_last_input.h` | частично | 6 | Инъекция provider и приоритет проверены; XCB/Mutter implementation отложена. |
| `platform/base_platform_layout_switch.h` | переведён | — | Linux false соответствует оригиналу, проверен тестом. |
| `platform/base_platform_network_reachability.h` | переведён | — | Платформенный интерфейс и Linux Create=None; тест общего слоя. QNetworkInformation — отдельный долг. |
| `platform/base_platform_power_save_blocker.h` | частично | 6 | Индексы типов, WindowId и backend trait; реальный Inhibit/XCB отложен. |
| `platform/base_platform_process.h` | отложен | 6 | Отложен: активация окна процесса, native WId и desktop connection. |
| `platform/base_platform_system_media_controls.h` | отложен | 7 | Отложен: MPRIS и команды медиаплеера, metadata/artwork, tg-gfx; Linux backend этап 6/7. |
| `platform/base_platform_url_scheme.h` | отложен | 6 | Отложен: desktop registration, Gio AppInfo, Flatpak/Snap и URL launcher. |

### platform/linux/ — пофайлово

| Файл | Решение | Этап возврата | Реализация / что требуется |
|---|---|---:|---|
| `platform/linux/base_battery_saving_linux.cpp` | отложен | 6 | GLib/GPowerProfileMonitor, notify callback через enterFromEventLoop; sysfs battery probe. |
| `platform/linux/base_battery_saving_linux.h` | отложен | 6 | GLib/GPowerProfileMonitor, notify callback через enterFromEventLoop; sysfs battery probe. |
| `platform/linux/base_file_utilities_linux.cpp` | частично | 6 | Проверены executable, rename overwrite, flush/sync, recursive delete (включая исходную ошибку symlink), Linux identity/no-op. ShowInFolder D-Bus/portal и BundledResourcesPath отложены. |
| `platform/linux/base_file_utilities_linux.h` | частично | 6 | Проверены executable, rename overwrite, flush/sync, recursive delete (включая исходную ошибку symlink), Linux identity/no-op. ShowInFolder D-Bus/portal и BundledResourcesPath отложены. |
| `platform/linux/base_global_shortcuts_linux.cpp` | отложен | 6 | XCB RECORD, xkbcommon имена клавиш, QSocketNotifier; источник событий tg-window. |
| `platform/linux/base_global_shortcuts_linux.h` | отложен | 6 | XCB RECORD, xkbcommon имена клавиш, QSocketNotifier; источник событий tg-window. |
| `platform/linux/base_haptic_linux.cpp` | отложен | 6 | feedbackd org.sigxcpu.Feedback, desktop application id; константа swipe находится в общем модуле. |
| `platform/linux/base_haptic_linux.h` | отложен | 6 | feedbackd org.sigxcpu.Feedback, desktop application id; константа swipe находится в общем модуле. |
| `platform/linux/base_info_linux.cpp` | отложен | 6 | DMI/sysfs, os-release/locale/uname, glibc, session backend, desktop environment. |
| `platform/linux/base_info_linux.h` | отложен | 6 | DMI/sysfs, os-release/locale/uname, glibc, session backend, desktop environment. |
| `platform/linux/base_last_input_linux.cpp` | отложен | 6 | MIT-SCREEN-SAVER XCB и Mutter IdleMonitor D-Bus, часы/приоритет fallback. |
| `platform/linux/base_last_input_linux.h` | отложен | 6 | MIT-SCREEN-SAVER XCB и Mutter IdleMonitor D-Bus, часы/приоритет fallback. |
| `platform/linux/base_layout_switch_linux.cpp` | переведён | — | Оригинальный false, unit test через общий модуль. |
| `platform/linux/base_layout_switch_linux.h` | переведён | — | Оригинальный false, unit test через общий модуль. |
| `platform/linux/base_linux_allocation_tracer.cpp` | отложен | 6 | Отложен: malloc hooks и адреса выделений, unsafe/FFI; безопасный tg-base не перехватывает allocator. |
| `platform/linux/base_linux_allocation_tracer.h` | отложен | 6 | Отложен: malloc hooks и адреса выделений, unsafe/FFI; безопасный tg-base не перехватывает allocator. |
| `platform/linux/base_linux_app_launch_context.cpp` | отложен | 6 | Gio AppLaunchContext, startup id/activation token и env. |
| `platform/linux/base_linux_app_launch_context.h` | отложен | 6 | Gio AppLaunchContext, startup id/activation token и env. |
| `platform/linux/base_linux_dbus_utilities.cpp` | отложен | 6 | NameHasOwner/ListActivatableNames/StartServiceByName и ServiceWatcher; допустим zbus, пока не добавлен. |
| `platform/linux/base_linux_dbus_utilities.h` | отложен | 6 | NameHasOwner/ListActivatableNames/StartServiceByName и ServiceWatcher; допустим zbus, пока не добавлен. |
| `platform/linux/base_linux_library.cpp` | отложен | 6 | dlopen/dlsym требуют unsafe: вынести FFI или заменить конечные API безопасными крейтами. |
| `platform/linux/base_linux_library.h` | отложен | 6 | dlopen/dlsym требуют unsafe: вынести FFI или заменить конечные API безопасными крейтами. |
| `platform/linux/base_linux_xcb_library.cpp` | отложен | 6 | Набор динамических XCB symbols; нужен безопасный X11 backend, tg-window, этап 5/6. |
| `platform/linux/base_linux_xcb_library.h` | отложен | 6 | Набор динамических XCB symbols; нужен безопасный X11 backend, tg-window, этап 5/6. |
| `platform/linux/base_linux_xcb_utilities.cpp` | отложен | 6 | Соединение, native event filter, atom/window/property, mask lifetime; tg-window X11 backend. |
| `platform/linux/base_linux_xcb_utilities.h` | отложен | 6 | Соединение, native event filter, atom/window/property, mask lifetime; tg-window X11 backend. |
| `platform/linux/base_linux_xdg_activation_token.cpp` | отложен | 6 | Wayland xdg activation и активное окно; tg-window stage 5. |
| `platform/linux/base_linux_xdg_activation_token.h` | отложен | 6 | Wayland xdg activation и активное окно; tg-window stage 5. |
| `platform/linux/base_linux_xdp_utilities.cpp` | отложен | 6 | Portal Settings watch, parent window export X11/Wayland; tg-window + D-Bus. |
| `platform/linux/base_linux_xdp_utilities.h` | отложен | 6 | Portal Settings watch, parent window export X11/Wayland; tg-window + D-Bus. |
| `platform/linux/base_linux_xsettings.cpp` | отложен | 6 | XSETTINGS property parser/cache, XCB events, QColor; tg-window + tg-gfx. |
| `platform/linux/base_linux_xsettings.h` | отложен | 6 | XSETTINGS property parser/cache, XCB events, QColor; tg-window + tg-gfx. |
| `platform/linux/base_network_reachability_linux.cpp` | переведён | — | Create=None действительно в оригинале; общий fallback QNetworkInformation сюда не относится. |
| `platform/linux/base_power_save_blocker_linux.cpp` | отложен | 6 | Portal Inhibit/Request.Close, parent window export и XCB screensaver reset раз в 10 с. |
| `platform/linux/base_power_save_blocker_linux.h` | отложен | 6 | Portal Inhibit/Request.Close, parent window export и XCB screensaver reset раз в 10 с. |
| `platform/linux/base_process_linux.cpp` | отложен | 6 | X11 _NET_ACTIVE_WINDOW/_NET_WM_USER_TIME, Wayland activation; нет window ids. |
| `platform/linux/base_process_linux.h` | отложен | 6 | X11 _NET_ACTIVE_WINDOW/_NET_WM_USER_TIME, Wayland activation; нет window ids. |
| `platform/linux/base_screen_reader_state_linux.cpp` | отложен | 6 | AT-SPI/org.a11y.Status, watchers и GLib callback delivery; accessibility stage 5. |
| `platform/linux/base_screen_reader_state_linux.h` | отложен | 6 | AT-SPI/org.a11y.Status, watchers и GLib callback delivery; accessibility stage 5. |
| `platform/linux/base_system_media_controls_linux.cpp` | отложен | 7 | MPRIS org.mpris.MediaPlayer2, playback/seek/artwork/URI metadata; player + gfx stage 7. |
| `platform/linux/base_system_unlock_linux.cpp` | переведён | — | Оригинальные known=true, available=false; Suggest немедленно Cancelled, unit tests. |
| `platform/linux/base_system_unlock_linux.h` | переведён | — | Оригинальные known=true, available=false; Suggest немедленно Cancelled, unit tests. |
| `platform/linux/base_url_scheme_linux.cpp` | отложен | 6 | Gio DesktopAppInfo, Snap settings, .desktop и sandbox; регистрация tg:// из launcher. |
| `platform/linux/base_url_scheme_linux.h` | отложен | 6 | Gio DesktopAppInfo, Snap settings, .desktop и sandbox; регистрация tg:// из launcher. |
| `platform/linux/io.snapcraft.Settings.xml` | отложен | 6 | Отложен: исходное описание D-Bus интерфейса для соответствующего backend; сохранить имена, типы и сигналы при переносе на zbus. |
| `platform/linux/mpris.xml` | отложен | 7 | Отложен: исходное описание D-Bus интерфейса для соответствующего backend; сохранить имена, типы и сигналы при переносе на zbus. |
| `platform/linux/org.a11y.Status.xml` | отложен | 6 | Отложен: исходное описание D-Bus интерфейса для соответствующего backend; сохранить имена, типы и сигналы при переносе на zbus. |
| `platform/linux/org.freedesktop.DBus.xml` | отложен | 6 | Отложен: исходное описание D-Bus интерфейса для соответствующего backend; сохранить имена, типы и сигналы при переносе на zbus. |
| `platform/linux/org.freedesktop.FileManager1.xml` | отложен | 6 | Отложен: исходное описание D-Bus интерфейса для соответствующего backend; сохранить имена, типы и сигналы при переносе на zbus. |
| `platform/linux/org.gnome.Mutter.IdleMonitor.xml` | отложен | 6 | Отложен: исходное описание D-Bus интерфейса для соответствующего backend; сохранить имена, типы и сигналы при переносе на zbus. |
| `platform/linux/org.sigxcpu.Feedback.Haptic.xml` | отложен | 6 | Отложен: исходное описание D-Bus интерфейса для соответствующего backend; сохранить имена, типы и сигналы при переносе на zbus. |

## Основание отложенных частей и границы проверки

Qt ownership helpers используются во всём lib_ui и window: QObject parent и deleteLater
являются частью контракта; простая замена на Box меняла бы момент уничтожения.
Qt signal bridge проходит через Integration::enterFromEventLoop; этот hook появится
с tg-window. Focus chain/Tab/modifiers нужны UI-вводу. UTF-16 StringViewMid и сравнение
QString нельзя объявить эквивалентом байтового &str. Эти файлы только проанализированы;
исполняемой реализации и притворных заглушек для них нет.

Linux backend-и используются в startup/launcher, calls push-to-talk, playback и accessibility:
ShowInFolder → FileManager1 → portal → desktop fallback, power-save → portal Inhibit/XCB,
last-input → XCB → Mutter, screen reader → org.a11y.Status/AT-SPI, media → MPRIS.
Они требуют живую desktop session, service ownership, parent-window export, native ids
и интеграцию FD/GLib callbacks с главной очередью. Эти ветви в данной headless-среде
запуском не проверены и не объявлены рабочими. XML интерфейсы перечислены отдельно,
не изменялись и не генерируют новый backend.

Тестами проверены только реализованные portable contracts, fake backend-и и локальный IPC;
чтением — соответствия Qt scheduling/JSON/local socket и отложенные интерфейсы.
Нативный Qt, настоящий перевод часов во время suspend, compositor, portals, GLib,
AT-SPI, X11 RECORD/MIT-SCREEN-SAVER, MPRIS и crash signals не запускались.

Отличия: callbacks Fn/FnMut и явные handles/registry, coarse timers не coalesce;
совпавшие deadlines упорядочены по старту (Qt не обещает стабильный порядок).
PowerSaveBlocker выбирает первый id вместо порядка адресов C++ и хранит WindowId,
автоматическое обнуление после уничтожения окна добавит tg-window.
Integration::ExecutableInfo использует std current_exe/canonicalize; invalid OS strings
преобразуются lossily, оригинал применял QFile decodeName. Crash header хранится отдельно,
не публикуется глобальным указателем; нечисловые double отвергаются явно.
JSON сохраняет поддержанные scalar types; точные тексты Qt ошибок и invalid Unicode
не воспроизводятся, имена options в клиенте ASCII. Неизвестные настройки игнорируются.

## Зависимости и нехватка готовых крейтов

Новые direct dependencies этой работы: `tg-runtime` (workspace 0.0.0) и
`serde_json = "1"` (разрешено 1.0.154 при проверке).
`tg-rpl` (0.0.0) уже был зависимостью. Остальное — std; для single_instance используется
существующий FileLock (nix на Linux), CRC32 из готового tg-base.
zbus/GLib/signal-hook не добавлены; решения о desktop backend оставлены до соответствующего этапа.

В tg-runtime нет deadline/alarm API и готовой интеграции reactor с оконной очередью:
адаптер Clock/Alarm находится в tg-base. Глобальные runtime часы не заменяются на
виртуальные; отдельный Clock в TimerHost и injected adjust decision покрывают тесты.
В tg-rpl нет QObject signal adapter и Send producers: производители остаются на главном
потоке; ConcurrentTimer использует Send runner и не переносит rpl между потоками.
Это границы готовых крейтов, менять их в этой задаче не потребовалось.

## Итоговая проверка 2026-11-12

Все команды выполнялись в Nix shell в worktree `base-runtime`, после слияния main.

```text
cargo fmt --check
(exit 0, stdout/stderr пусты)

cargo clippy --workspace --all-targets -- -D warnings
Checking tg-base v0.0.0
Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.68s
(exit 0, предупреждений нет; время включает ожидание cargo build lock)

cargo test --workspace
424 passed; 0 failed; 0 ignored (сумма верхнеуровневых test binaries и doctests)
tg-base: 208 unit + 22 integration + 1 doctest = 231
(exit 0)
```

Subprocess helper single_instance выполнен дополнительно в дочернем процессе и
не посчитан повторно в 424. В исходном main до этой работы у tg-base было 142
unit tests; добавлено 66 unit и 9 integration (всего +75 проверок).
Полные локальные логи: `/tmp/base-runtime-verified-fmt.log`,
`/tmp/base-runtime-verified-clippy.log`, `/tmp/base-runtime-verified-tests.log`.
