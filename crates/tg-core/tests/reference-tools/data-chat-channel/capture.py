#!/usr/bin/env python3
"""Capture actual native output with pinned source hashes, never user data."""
import argparse
import collections
import hashlib
import json
from pathlib import Path
import subprocess

p = argparse.ArgumentParser()
p.add_argument('executable', type=Path)
p.add_argument('telegram_root', type=Path)
p.add_argument('scheme_include', type=Path)
p.add_argument('output', type=Path)
a = p.parse_args()
pinned = '0f2a31bb420de98f331990432265b131b8d836b5'
head = subprocess.check_output(['git', '-C', str(a.telegram_root), 'rev-parse', 'HEAD'], text=True).strip()
assert head == pinned, (head, pinned)
first = subprocess.check_output([str(a.executable.resolve())])
assert first == subprocess.check_output([str(a.executable.resolve())]), 'native output is not deterministic'
a.output.mkdir(parents=True, exist_ok=True)
(a.output / 'pure.csv').write_bytes(first)
files = ['SourceFiles/data/data_chat.h', 'SourceFiles/data/data_chat.cpp', 'SourceFiles/data/data_channel.h', 'SourceFiles/data/data_channel.cpp', 'SourceFiles/data/data_channel_admins.h', 'SourceFiles/data/data_channel_admins.cpp', 'SourceFiles/data/data_chat_participant_status.h', 'SourceFiles/data/data_chat_participant_status.cpp', 'SourceFiles/ayu/utils/telegram_helpers.cpp', 'lib_base/base/flags.h', 'lib_tl/tl/tl_basic_types.cpp']

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
manifest = {
    'original_revision': pinned,
    'toolchain': 'GCC 15.3.0, Linux x86_64, -O2, Qt Core 6.11.2, C++23',
    'scope': 'rights conversions, send unions, original normalization prelude, forever predicate, Qt Zalgo only; full models/Session are own tests + reading',
    'unicode': 'Mn property captured from Qt QRegularExpression/PCRE, every scalar 0..10FFFF',
    'rows': len(first.splitlines()),
    'kinds': dict(sorted(collections.Counter(line.split(b',', 1)[0].decode() for line in first.splitlines()).items())),
    'sha256': digest(a.output / 'pure.csv'),
    'sources_sha256': {f: digest(a.telegram_root / f) for f in files},
    'generated_sha256': {f: digest(a.scheme_include / f) for f in ['scheme.h', 'scheme.cpp']},
}
(a.output / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(f'native: {manifest["rows"]} deterministic rows; sha256 {manifest["sha256"]}')
