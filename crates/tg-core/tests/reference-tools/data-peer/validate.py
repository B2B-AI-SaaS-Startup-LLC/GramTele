#!/usr/bin/env python3
"""Check committed proof metadata and nonempty, test-addressed function map."""
import collections
import hashlib
import json
from pathlib import Path
import re

root = Path(__file__).resolve().parents[3]
fixture = root / 'tests/fixtures/data-peer'
m = json.loads((fixture / 'manifest.json').read_text())
data = (fixture / 'pure.csv').read_bytes()
assert hashlib.sha256(data).hexdigest() == m['sha256']
assert len(data.splitlines()) == m['rows']
assert dict(collections.Counter(l.split(b',', 1)[0].decode() for l in data.splitlines())) == m['kinds']
tests = set(re.findall(r'^fn (\w+)\(', (root / 'tests/data_peer.rs').read_text(), re.M))
rows = [l for l in (root / 'docs/data-peer-map.md').read_text().splitlines() if re.match(r'\| (data|ui|core)/', l)]
assert len(rows) == 357, len(rows)
counts = collections.Counter()
for row in rows:
    columns = re.split(r'(?<!\\)\|', row)[1:-1]
    assert len(columns) == 5, row
    source, function, status, implementation, test = [c.strip() for c in columns]
    assert all([source, function, status, implementation, test])
    counts[status.split('(')[0]] += 1
    assert status == 'done' or re.fullmatch(r'partial\([0-9/]+\)', status), row
    used = re.findall(r'\b[a-z]+(?:_[a-z0-9]+)+\b', test)
    assert used and all(n in tests for n in used), (source, used)
assert counts == {'done': 300, 'partial': 57}, counts
print(f'proof PASS: {m["rows"]} native rows; map {len(rows)} rows, {dict(counts)}, 0 empty links')
