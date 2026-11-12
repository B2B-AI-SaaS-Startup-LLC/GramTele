# Источники fixtures

`Telegram/` — неизменённые настоящие входы из AyuGram Desktop
`0f2a31bb420de98f331990432265b131b8d836b5`: все 76 .style, colors.palette,
lang.strings, emoji.txt, emoji_autocomplete.json и используемые иконки со
scale variants. Пути сохраняют структуру исходного Telegram. SHA256 каждого
снимка — SHA256.json. Исходные copyright/license notices сохранены; применимы
лицензии соответствующих файлов оригинала (в частности Desktop App Toolkit:
https://github.com/desktop-app/legal/blob/master/LEGAL).

`reference/` — **вывод реально запущенного оригинала**, а не повторная реализация:

- data.json: числа из C++ массивов оригинальных codegen_lang/codegen_emoji,
  palette compute rows и checksum из codegen_style;
- lang_auto.tags/lang_auto.indices: точные state files чистой генерации;
- style-ir.json: данные оригинального ParsedFile/Module/Value по 76 стилям;
- emoji-ir.json: оригинальный PrepareData, старые aliases, postfixRequired,
  emoticon replacements;
- original-output-sha256.json: SHA256 всех сохранённых original output files.

Сборка: GCC 15.3.0, C++23, Qt 6.11.2, Nix; исходники не изменялись.
Отдельный build в /tmp/ayugram-codegen-reference. Обычный CMake корня не
конфигурировался из-за отсутствующих cmake submodule files; standalone targets
собраны из тех же .cpp/.h. PCH заменён только стандартными includes и typedefs.

Воспроизведение (вне клона, каталог вывода эталонов выбрать явно):

```bash
nix-shell -p cmake ninja qt6.qtbase qt6.qtsvg python3 --run \
  'bash crates/tg-codegen/tests/reference-tools/capture.sh /path/to/AyuGramDesktop /tmp/codegen-reference /tmp/new-reference'
```

Не перезаписывайте checked-in эталоны выходом самого Rust генератора.
Компиляционные contracts в tests/support — только формы API, не runtime UI,
не проверка пикселей и не эталон данных.
