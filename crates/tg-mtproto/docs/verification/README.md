# Проверка 2026-11-12

Rust: `1.95.0 (59807616e 2026-04-14)`, закреплённый flake.lock.
Worktree: port/tg-mtproto-transport от main 9f6ead6.

```bash
nix develop --command bash -euc 'cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace'
```

| Команда | Код выхода | Вывод |
|---|---|---|
| cargo fmt --check | 0 | Вывода нет. |
| cargo clippy --workspace --all-targets -- -D warnings | 0 | Finished dev profile, без замечаний. |
| cargo test --workspace | 0 | 515 passed, 0 failed, 2 ignored (live). |

Полный stdout/stderr — [workspace.log](workspace.log).
В tg-mtproto 55 обычных tests, все прошли. В tg-scheme новый native TL test
проверяет 15 120 образцов 2520 определений, второй — 3 образца builtin vector.
Число образцов не выдаётся за число test functions.

Live command:

```bash
nix develop --command cargo test -p tg-mtproto --test live_test_dc -- --ignored --nocapture
```

Код выхода **101**, 1 passed / 1 failed. TCP test DC 2 успешно завершил DH;
keyId в последнем запуске `4c0cabdf83e09833`. HTTP получил Parse(Status).
Raw req_pq_multi probe получил строку `HTTP/1.0  200 OK` с двумя пробелами.
Сервер отвечает; сообщение «сети нет» было бы неверным. Полный вывод —
[live-test-dc.log](live-test-dc.log). Production DC и API/account не использованы.

Native reference и свои tests отделены в [README крейта](../../README.md),
решение по всем 49 исходным файлам — там же и в [files.json](../files.json).
Нативные fixtures совпали без обнаруженных расхождений в описанном наборе;
это не доказательство паритета OAEP, всех ASN.1/IDNA/error paths и всех Qt
floating-point текстовых представлений. Internet proxy/fake TLS не запускались.
