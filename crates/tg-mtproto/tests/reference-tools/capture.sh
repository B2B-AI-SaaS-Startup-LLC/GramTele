#!/usr/bin/env bash
set -euo pipefail
original=${1:?path to AyuGramDesktop clone}
output=${2:?external writable capture directory}
tl_reference=${3:?external native TL build directory from tg-scheme capture}
here=$(cd -- "$(dirname -- "$0")" && pwd)
mkdir -p "$output"
scheme_object=$(rg --files --hidden --no-ignore "$tl_reference/build" | rg '/scheme.cpp.o$' | head -n 1)
python3 "$here/extract_tcp.py" "$original/Telegram" "$output/tcp.cpp"
python3 "$here/extract_tls.py" "$original/Telegram" "$output/tls.cpp"
cmake -S "$here" -B "$output/build" -G Ninja \
 -DTELEGRAM_ROOT="$original/Telegram" -DREFERENCE_DIR="$output" \
 -DSCHEME_INCLUDE="$tl_reference/gen" -DSCHEME_OBJECT="$scheme_object"
cmake --build "$output/build" -j2
"$output/build/native_crypto" "$output/crypto.bin" "$output/storage-custom.bin" "$output/ige.bin"
for name in tcp tls socks connect proxy; do
 "$output/build/native_$name" "$output/$name.bin"
done
