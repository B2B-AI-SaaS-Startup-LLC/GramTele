#!/usr/bin/env python3
"""Rebuild independent evidence in new synthetic directories, never user tdata."""
import argparse
import hashlib
import json
import re
from pathlib import Path
import shutil
import subprocess
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument("native", type=Path)
parser.add_argument("--workspace", type=Path, default=Path(__file__).resolve().parents[4])
parser.add_argument("--update-manifest", action="store_true")
args = parser.parse_args()
workspace = args.workspace.resolve()
fixtures = workspace / "crates/tg-cache/tests/fixtures"
native = args.native.resolve()
cache=(native.parent/"CMakeCache.txt").read_text()
telegram=Path(re.search(r"^TELEGRAM_ROOT:[^=]+=(.+)$",cache,re.MULTILINE).group(1))

def run(*command):
    subprocess.run([str(arg) for arg in command], cwd=workspace, check=True)

def tree(root):
    return {str(p.relative_to(root)): p.read_bytes() for p in root.rglob("*") if p.is_file()}

def equal(left, right, label):
    a, b = tree(left), tree(right)
    differences = [p for p in sorted(a.keys() | b.keys()) if a.get(p) != b.get(p)]
    if differences:
        raise SystemExit(f"{label}: differing paths: {differences}")
    print(f"{label}: PASS ({len(a)} files, {sum(map(len,a.values()))} bytes)", flush=True)

with tempfile.TemporaryDirectory(prefix="ayugram-cache-database-reference-") as temp:
    root = Path(temp)
    run(native, "generate", root / "native")
    equal(root / "native", fixtures / "database", "native fixture regeneration")
    run("nix", "develop", "--command", "cargo", "run", "-p", "tg-cache", "--example", "cache_database_reference", "--", "generate", root / "rust")
    equal(root / "native", root / "rust", "native -> Rust exact writer bytes")
    shutil.copytree(root / "native", root / "native-to-rust")
    run("nix", "develop", "--command", "cargo", "run", "-p", "tg-cache", "--example", "cache_database_reference", "--", "verify", root / "native-to-rust")
    shutil.copytree(root / "rust", root / "rust-to-native")
    run(native, "verify", root / "rust-to-native")
    equal(root / "native-to-rust", root / "rust-to-native", "reader side effects in both directions")
    run(native, "probes", root / "probes")
    equal(root / "probes", fixtures / "database-probes", "22 native recovery branches")
    result = subprocess.check_output([str(native), "faults", str(root / "faults")])
    if result != (fixtures / "file-faults.csv").read_bytes():
        raise SystemExit("native QFile fault results differ")
    print("native partial write / ENOSPC / flush: PASS", flush=True)
    cleaner = subprocess.check_output([str(native), "cleaner", str(root / "cleaner")])
    if cleaner != (fixtures / "cleaner-check.csv").read_bytes():
        raise SystemExit("native cleaner symlink results differ")
    print("native cleaner root/nested symlinks: PASS", flush=True)
    manifest = {
        "original": "0f2a31bb420de98f331990432265b131b8d836b5",
        "lib_storage": "ccdc72548a5065b5991b4e06e610d76bc4f6023e",
        "harness": "Qt 6.11.2 / OpenSSL 3.6.5 / GCC 15.3, Linux x86_64",
        "key": "64 bytes: 00..3f", "unix_time": "1700000000; explicit per-operation changes in main.cpp",
        "entropy": "byte counter (wide: counter XOR counter>>8 XOR counter>>16)",
        "files": {str(p.relative_to(fixtures)): hashlib.sha256(p.read_bytes()).hexdigest() for folder in ("database", "database-probes") for p in sorted((fixtures / folder).rglob("*")) if p.is_file()},
        "faults": hashlib.sha256(result).hexdigest(),
        "cleaner": hashlib.sha256(cleaner).hexdigest(),
        "source_files": {str(p.relative_to(telegram)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted((telegram/"lib_storage/storage").rglob("*")) if p.is_file() and p.suffix in (".cpp",".h")},
        "toolkit_commits": {name:subprocess.check_output(["git","-C",str(telegram/name),"rev-parse","HEAD"],text=True).strip() for name in ("lib_storage","lib_base","lib_crl","lib_rpl")},
    }
    output = workspace / "crates/tg-cache/tests/database-reference/manifest.json"
    encoded = json.dumps(manifest, indent=2, ensure_ascii=False) + "\n"
    if output.exists() and output.read_text() != encoded and not args.update_manifest:
        raise SystemExit("reference manifest changed; inspect and explicitly update it")
    output.write_text(encoded)
    print("bidirectional original-code reference: PASS", flush=True)
