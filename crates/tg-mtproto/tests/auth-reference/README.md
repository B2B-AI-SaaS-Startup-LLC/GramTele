# Нативная проверка запросов входа

CMake связывает оригинальный lib_tl и объект scheme.cpp, полученный существующим
native TL harness tg-scheme из generate_tl.py/codegen_scheme.py оригинала.
Передаются TELEGRAM_ROOT, SCHEME_INCLUDE (с scheme.h), SCHEME_OBJECT
(скомпилированный scheme.cpp.o), Qt6 Core/Gui/Network. Сборка находится вне клона.

`native_auth input.json output.bin`: вход — phone, signin_phone (по умолчанию phone),
hash, code. Записывает исходным C++ три пакета: auth.sendCode/codeSettings flags=0,
auth.signIn flags=phone_code и invokeWithLayer(229)/initConnection с первым запросом.
Каждый пакет предварён u32 LE длины в байтах. API 17349/hash из config.h;
Desktop/Linux/7.2.9/en/tdesktop/en, params tz_offset=0 — параметры тестового клиента.

`../fixtures/auth-login/cases.json` — описания семи фактических диагностических пар
(попытки 2–8). cases.tsv передаёт те же значения Rust-тесту без дополнительного
JSON-крейта в tg-mtproto; cases.bin — конкатенация записей оригинального C++.
Смещения/длины указаны в описании. Prefix words взяты из фактической Session
попыток 3–8; в попытке 2 они ещё не регистрировались. Запросы из полных логов
сопоставлены с первыми двумя пакетами побайтно. Первая попытка старым бинарником
включена в лимит входов, но без доказательного дампа в корпус не добавлена.

Rust-тест: `cargo test -p tg-mtproto --test auth_login_reference`.
Эта программа не подключается к сети и не выполняет вход оригинальным клиентом.
