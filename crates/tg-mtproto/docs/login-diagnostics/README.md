# Разбор PHONE_CODE_INVALID

Оригинал: 0f2a31bb420de98f331990432265b131b8d836b5. Только синтетические номера,
тестовые DC, API 17349/hash из config.h. Использовано 6 из разрешённых 8 попыток;
FLOOD_WAIT не получен. Попытка 1 запущена старым бинарником до окончания сборки,
в бюджет включена, диагностическим доказательством не считается.

| Попытка | Номер | DC | Код | Результат |
|---|---|---|---|---|
| 1 | 9996624442 | 2 | 22222 | PHONE_CODE_INVALID (старый бинарник) |
| 2 | 9996629716 | 2 | 22222 | PHONE_CODE_INVALID |
| 3 | 9996613177 | 1 | 11111 | PHONE_CODE_INVALID |
| 4 | 9996638502 | 3 | 33333 | PHONE_CODE_INVALID |
| 5 | 9996621689 | 2 | 22222 | PHONE_CODE_INVALID, nearest/countries, плюс в sendCode |
| 6 | 9996624880 | 2 | 22222 | PHONE_CODE_INVALID, nearest/countries + suggestMainDcId |

Полные дампы — в attempt*.log. Во всех диагностических попытках sendCode вернул
sentCodeTypeSms(length=5), next_type=Call, timeout=90; почта и иной шаг не требуются.
Нативный `tests/auth-reference` использует оригинальный lib_tl и C++-схему:
sendCode и signIn попытки 2 совпадают побайтно, включая codeSettings flags=0
и signIn flags=phone_code. Также проверена полная обёртка invokeWithLayer(229)
и initConnection с API 17349, Desktop/Linux, 7.2.9, en/tdesktop/en, tz_offset=0.
Эти же слова префикса зарегистрированы фактической сессией в логах. В Linux
оригинал получает модель из DMI/virtualization, версию из uname/desktop; Desktop
и Linux — явные параметры примера, не утверждение о фактической модели Qt.

Сессия и auth_key_id между sendCode и signIn одинаковы. Подтверждены тестовые IP:
DC1 149.154.175.10, DC2 149.154.167.40, DC3 149.154.175.117. Содержимое ключа не логируется.

По чтению intro_phone.cpp: cancelNearestDcRequest, setUserPhone, sendCode с
пустым codeSettings; intro_code.cpp передаёт DigitsOnly(phone), тот же hash
и введённые цифры. fullNumber в sendCode может содержать плюс/пробелы UI;
попытки 5–6 проверяют плюс. IntroWidget асинхронно запрашивает getNearestDc,
CountriesManager — getCountriesList с пустым языком/hash=0, если нет кэша.
Это независимые запросы UI, строгого обязательного порядка перед sendCode нет.
Пример первоначально их пропускал, повтор с ними результата не изменил.

Причина не установлена. Ни отказ сервера поддерживать тестовые номера, ни
корректность всего клиентского обмена этим не доказаны. Дополнительные попытки
не выполняются; QR и проверка tdata новым процессом остаются заблокированы
неуспешным входом. Правило номера/кода проверено по https://core.telegram.org/api/auth.
