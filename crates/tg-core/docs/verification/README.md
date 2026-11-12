# Проверка tdata, 2026-11-12/13

workspace.log — окончательная команда PORTING.md, Rust 1.95.0:
fmt exit 0; clippy --workspace --all-targets -- -D warnings exit 0;
test --workspace exit 0: 595 passed, 0 failed, 3 ignored. Кратко — summary.txt.

native-create.log и native-read-rust.log — четыре варианта каталога, original
write/read и original read Rust. original-to-rust.log — первый checkpoint
побайтового сравнения Rust, повторённый в окончательном workspace test. Native
build.log собирает utility/domain/auth harness. native-qt-pbkdf-checkpoint.log —
первый запуск дополнительных vectors; после добавления narrow authorization
весь набор повторён окончательным cargo test. Границы native execution — в
README крейта и tests/reference-tools/README.md; полный клиент не запускался.

login-initial-metadata-error.log — отвергнутый пустой device_model примера.
login-test-dc.log — исправленный Desktop/Linux, sendCode length=5 и
PHONE_CODE_INVALID на 22222 для 9996620387. После этого живые попытки не
повторялись. В этих ранних запусках ещё был включён automatic help.getConfig;
в окончательном коде примеры отключают его, что подтверждено отдельным тестом.

Успешного входа, QR и живого нового процесса с tdata не было. Их результаты
не подменены native/synthetic tests. Аккаунт не авторизован, production DC и
настоящая tdata пользователя не использовались. Пуш не выполнялся.
