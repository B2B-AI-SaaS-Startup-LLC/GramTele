#!/usr/bin/env bash
# Строит реестр портирования: каждый исходный файл оригинала -> крейт и этап.
# Использование: tools/gen-ledger.sh <путь к AyuGramDesktop> > ledger/files.csv
# Перезаписывает статусы: запускать только для первичной сборки реестра
# или при смене исходной точки.
set -euo pipefail
src="${1:?путь к клону AyuGramDesktop}"
cd "$src/Telegram"

classify() {
	# $1 - путь относительно Telegram/. Печатает "крейт,этап".
	case "$1" in
	lib_icu/*|SourceFiles/ayu/libs/*) echo "-,-" ;;
	lib_base/*) echo "tg-base,1" ;;
	lib_crl/*) echo "tg-runtime,1" ;;
	lib_rpl/*) echo "tg-rpl,1" ;;
	lib_tl/*) echo "tg-tl,1" ;;
	codegen/*) echo "tg-codegen,1" ;;
	SourceFiles/mtproto/*) echo "tg-mtproto,2" ;;
	lib_storage/*) echo "tg-cache,3" ;;
	SourceFiles/storage/*) echo "tg-core,3" ;;
	SourceFiles/ayu/data/*|SourceFiles/ayu/ayu_settings.*|SourceFiles/ayu/ayu_state.*) echo "ayu-core,3" ;;
	SourceFiles/history/history.cpp|SourceFiles/history/history.h|SourceFiles/history/history_item*) echo "tg-core,4" ;;
	SourceFiles/main/*|SourceFiles/data/*|SourceFiles/api/*|SourceFiles/core/*|SourceFiles/lang/*|SourceFiles/countries/*) echo "tg-core,4" ;;
	SourceFiles/apiwrap.*|SourceFiles/config.h|SourceFiles/settings.*|SourceFiles/logs.*|SourceFiles/stdafx.*) echo "tg-core,4" ;;
	SourceFiles/ayu/utils/*|SourceFiles/ayu/features/filters/*|SourceFiles/ayu/ayu_worker.*|SourceFiles/ayu/ayu_infra.*|SourceFiles/ayu/ayu_lang.*|SourceFiles/ayu/ayu_url_handlers.*) echo "ayu-core,4" ;;
	lib_ui/*) echo "tg-ui,5" ;;
	lib_lottie/*) echo "tg-lottie,5" ;;
	lib_qr/*) echo "tg-qr,5" ;;
	SourceFiles/ui/*|SourceFiles/layout/*) echo "tg-app-ui,5" ;;
	SourceFiles/platform/linux/*) echo "tg-platform,6" ;;
	SourceFiles/platform/win/*|SourceFiles/platform/mac/*) echo "tg-platform,10" ;;
	SourceFiles/platform/*) echo "tg-platform,6" ;;
	lib_spellcheck/*) echo "tg-spellcheck,6" ;;
	SourceFiles/window/*|SourceFiles/intro/*|SourceFiles/dialogs/*) echo "tg-app,6" ;;
	SourceFiles/mainwidget.*|SourceFiles/mainwindow.*|SourceFiles/tray*|SourceFiles/main.cpp) echo "tg-app,6" ;;
	SourceFiles/history/*|SourceFiles/chat_helpers/*|SourceFiles/media/*|SourceFiles/poll/*|SourceFiles/inline_bots/*|SourceFiles/menu/*) echo "tg-app,7" ;;
	SourceFiles/ffmpeg/*) echo "tg-media,7" ;;
	SourceFiles/boxes/*|SourceFiles/info/*|SourceFiles/settings/*|SourceFiles/overview/*|SourceFiles/profile/*) echo "tg-app,8" ;;
	lib_translate/*) echo "tg-platform,8" ;;
	SourceFiles/ayu/*) echo "ayu-app,8" ;;
	SourceFiles/calls/*) echo "tg-calls,9" ;;
	lib_webrtc/*) echo "tg-webrtc,9" ;;
	SourceFiles/tde2e/*) echo "tg-e2e,9" ;;
	lib_webview/*) echo "tg-webview,9" ;;
	SourceFiles/iv/*) echo "tg-iv,9" ;;
	SourceFiles/editor/*) echo "tg-editor,9" ;;
	SourceFiles/export/*) echo "tg-export,9" ;;
	SourceFiles/passport/*|SourceFiles/payments/*|SourceFiles/statistics/*|SourceFiles/webauthn/*|SourceFiles/support/*) echo "tg-app,9" ;;
	SourceFiles/_other/*) echo "tg-tools,10" ;;
	SourceFiles/test/*|SourceFiles/tests/*) echo "tests,11" ;;
	*) echo "UNMAPPED,?" ;;
	esac
}

echo "path,lines,crate,stage,status"
find SourceFiles lib_* codegen -type f \( -name '*.cpp' -o -name '*.h' -o -name '*.mm' \) \
	-not -path '*/ThirdParty/*' -not -path '*/.git/*' | LC_ALL=C sort | while read -r f; do
	n=$(wc -l < "$f")
	m=$(classify "$f")
	if [ "${m%%,*}" = "-" ]; then st="vendored"; else st="todo"; fi
	echo "$f,$n,$m,$st"
done
