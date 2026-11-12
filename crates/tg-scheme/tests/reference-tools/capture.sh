#!/usr/bin/env bash
set -euo pipefail
original=${1:?path to AyuGramDesktop clone}
output=${2:?external writable build directory}
here=$(cd -- "$(dirname -- "$0")" && pwd)
mkdir -p "$output/gen"
python3 "$original/Telegram/SourceFiles/codegen/scheme/codegen_scheme.py" \
  "-o$output/gen/scheme" "$original/Telegram/SourceFiles/mtproto/scheme/api.tl" \
  "$original/Telegram/SourceFiles/mtproto/scheme/mtproto.tl"
python3 "$here/describe.py" "$original/Telegram/SourceFiles/mtproto/scheme" "$output/recipe.json"
python3 "$here/emit.py" "$output/recipe.json" "$output"
cmake -S "$here" -B "$output/build" -G Ninja \
  -DTELEGRAM_ROOT="$original/Telegram" -DREFERENCE_DIR="$output"
cmake --build "$output/build" -j1
"$output/build/native_tl_reference" "$output/reference.bin"
"$output/build/native_tl_vectors" "$output/vectors.bin"
echo "Reference files: $output/{recipe.json,builders.rs,reference.bin}"
