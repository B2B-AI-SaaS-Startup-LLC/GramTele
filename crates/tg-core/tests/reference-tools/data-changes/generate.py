#!/usr/bin/env python3
"""Record actual original code at both optimization levels; never patch it."""
import argparse
import hashlib
import json
import platform
import subprocess
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("build", type=Path)
parser.add_argument("output", type=Path)
parser.add_argument("--telegram-root", required=True, type=Path)
parser.add_argument("--scheme-include", required=True, type=Path)
args = parser.parse_args()
revision = subprocess.check_output(
    ["git", "-C", str(args.telegram_root), "rev-parse", "HEAD"], text=True
).strip()
assert revision == "0f2a31bb420de98f331990432265b131b8d836b5", revision
fixtures = {}
for name, command in [("peer-shifts.csv", []), ("managers.csv", ["managers"]), ("flags.csv", ["flags"])]:
    runs = [subprocess.check_output([str(args.build.resolve() / ("native_data_changes_" + mode)), *command]) for mode in ["O0", "O2"]]
    assert runs[0] == runs[1], f"{name}: O0 differs from O2"
    fixtures[name] = runs[0]
files = ["SourceFiles/data/data_changes.h", "SourceFiles/data/data_changes.cpp",
         "SourceFiles/data/data_chat_participant_status.h", "SourceFiles/main/main_session.h",
         "lib_base/base/flags.h", "lib_base/base/flat_map.h", "lib_rpl/rpl/event_stream.h"]
sha = lambda data: hashlib.sha256(data).hexdigest()
cache = (args.build / "CMakeCache.txt").read_text()
compiler = next(line.split("=", 1)[1] for line in cache.splitlines() if line.startswith("CMAKE_CXX_COMPILER:FILEPATH="))
manifest = {
    "revision": revision,
    "compiler": subprocess.check_output([compiler, "--version"], text=True).splitlines()[0],
    "platform": platform.system() + "/" + platform.machine(),
    "optimization_levels": ["-O0", "-O2"],
    "sources_sha256": {name: sha((args.telegram_root / name).read_bytes()) for name in files},
    "scheme_h_sha256": sha((args.scheme_include / "scheme.h").read_bytes()),
    "fixtures": {name: {"lines": data.count(b"\n"), "sha256": sha(data)} for name, data in fixtures.items()},
    "scope": "real Manager bodies for 7 update types + real Changes ctor/sendNotifications; no Main::Session lifecycle or crl scheduling",
}
args.output.mkdir(parents=True, exist_ok=True)
for name, data in fixtures.items():
    (args.output / name).write_bytes(data)
(args.output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
