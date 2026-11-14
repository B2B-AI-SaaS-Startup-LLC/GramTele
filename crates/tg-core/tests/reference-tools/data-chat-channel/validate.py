#!/usr/bin/env python3
"""Validate map references, native proof hash and literal Qt Mn table."""
from pathlib import Path
import collections
import hashlib
import json
import re
root=Path(__file__).resolve().parents[3]
f=root/'tests/fixtures/data-chat-channel'
m=json.loads((f/'manifest.json').read_text());data=(f/'pure.csv').read_bytes()
assert hashlib.sha256(data).hexdigest()==m['sha256']
assert len(data.splitlines())==m['rows']==481
assert dict(collections.Counter(l.split(b',',1)[0].decode()for l in data.splitlines()))==m['kinds']
expected=[tuple(map(int,l.split(b',')[1:]))for l in data.splitlines()if l.startswith(b'mn,')]
rust=root.joinpath('src/ayu/utils/telegram_helpers.rs').read_text()
actual=[tuple(int(x,16)for x in pair)for pair in re.findall(r'\(0x([0-9a-f]+), 0x([0-9a-f]+)\)',rust)]
assert actual==expected,(len(actual),len(expected))
tests=set(re.findall(r'^fn (\w+)\(',root.joinpath('tests/data_chat_channel.rs').read_text(),re.M))
rows=[l for l in root.joinpath('docs/data-chat-channel-map.md').read_text().splitlines()if re.match(r'\| (data|ayu)/',l)]
assert len(rows)==337,len(rows)
counts=collections.Counter()
for row in rows:
 cols=[c.strip()for c in re.split(r'(?<!\\)\|',row)[1:-1]]
 assert len(cols)==5 and all(cols),row
 status=cols[2];assert status=='done'or re.fullmatch(r'partial\([0-9/]+\)',status),row
 counts[status.split('(')[0]]+=1
 used=re.findall(r'\b[a-z]+(?:_[a-z0-9]+)+\b',cols[4]);assert used and all(n in tests for n in used),(cols[0],used)
assert counts=={'done':287,'partial':50},counts
print(f'proof/map PASS: {len(rows)} rows {dict(counts)}, 0 empty; native481, Mn{len(expected)} ranges')
