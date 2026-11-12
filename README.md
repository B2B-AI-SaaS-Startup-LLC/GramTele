# ayugram-rs

Порт AyuGram Desktop (C++/Qt) на Rust. Исходная точка — AyuGram Desktop 7.2.9; оригинал только читается.

- [PLAN.md](PLAN.md) — объём, этапы, технические замены.
- [PORTING.md](PORTING.md) — правила порта и проверки.
- [ledger/files.csv](ledger/files.csv) — реестр: по строке на каждый файл оригинала (крейт, этап, статус).

## Окружение

Rust берётся из Nix:

```bash
nix-shell            # или: nix develop
```

## Проверки

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Лицензия: GPL-3.0-or-later, с исключением для OpenSSL (см. [LICENSE](LICENSE)).

<img src="secret-o3-api.jpg" alt="Сэм Альтман" width="670">
