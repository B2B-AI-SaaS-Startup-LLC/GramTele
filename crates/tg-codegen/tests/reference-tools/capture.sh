#!/usr/bin/env bash
set -euo pipefail
# Run inside a Nix shell with cmake, ninja, qt6.qtbase, qt6.qtsvg and python3.
# Usage: capture.sh AYUGRAM_SOURCE WORK_DIR FIXTURE_DESTINATION
source_root=$(realpath "$1")
work_dir=$(realpath -m "$2")
fixtures=$(realpath -m "$3")
tools_dir=$(cd -- "$(dirname -- "$0")" && pwd)
mkdir -p "$work_dir/build" "$work_dir/output/"{styles,palette,lang,emoji}
cmake -S "$tools_dir" -B "$work_dir/build" -G Ninja -DAYUGRAM_SOURCE="$source_root"
cmake --build "$work_dir/build" --target codegen_style codegen_lang codegen_emoji -j 2
# Dump targets reuse unchanged objects; configure again after they exist.
cmake -S "$tools_dir" -B "$work_dir/build" -DAYUGRAM_SOURCE="$source_root"
cmake --build "$work_dir/build" --target style_ir_dump emoji_ir_dump -j 2
resources="$source_root/Telegram"
style_args=(-I"$resources/SourceFiles" -I"$resources/lib_ui" -I"$resources/Resources")
"$work_dir/build/codegen_style" "${style_args[@]}" -o"$work_dir/output/palette" -t"$work_dir/output/palette/stamp" "$resources/lib_ui/ui/colors.palette"
"$work_dir/build/codegen_lang" -o"$work_dir/output/lang" "$resources/Resources/langs/lang.strings"
"$work_dir/build/codegen_emoji" -o"$work_dir/output/emoji" "$resources/lib_ui/emoji.txt" "$resources/lib_ui/emoji_suggestions/emoji_autocomplete.json"
mapfile -d '' inputs < <(find "$resources" -name '*.style' -print0 | sort -z)
"$work_dir/build/codegen_style" "${style_args[@]}" -o"$work_dir/output/styles" -t"$work_dir/output/styles/stamp" "${inputs[@]}"
"$work_dir/build/style_ir_dump" "${style_args[@]}" -o"$work_dir/output/styles" -t"$work_dir/output/styles/stamp" "${inputs[@]}"
"$work_dir/build/emoji_ir_dump" -o"$work_dir/output/emoji" "$resources/lib_ui/emoji.txt" "$resources/lib_ui/emoji_suggestions/emoji_autocomplete.json"
python3 "$tools_dir/extract.py" "$work_dir/output" "$fixtures" "$source_root"
