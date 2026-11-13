"""Rebuild synthetic cache fixtures using the separately compiled C++ original."""
import argparse
import hashlib
import json
import shutil
import subprocess
import tempfile
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("native", type=Path)
parser.add_argument("output", type=Path)
parser.add_argument("--telegram-root", type=Path, required=True)
args = parser.parse_args()
original = "0f2a31bb420de98f331990432265b131b8d836b5"
actual = subprocess.check_output(
    ["git", "-C", str(args.telegram_root), "rev-parse", "HEAD"], text=True
).strip()
if actual != original:
    raise SystemExit(f"Expected original {original}, got {actual}")
if Path("/dev/shm").stat().st_dev == Path(tempfile.gettempdir()).stat().st_dev:
    raise SystemExit("The native cross-filesystem case requires /dev/shm on a different filesystem")
args.output.mkdir(parents=True, exist_ok=True)
native = str(args.native.resolve())
with tempfile.TemporaryDirectory(prefix="ayu-cache-native-") as folder:
    work = Path(folder)
    subprocess.run([native, "fixtures", str(work / "fixtures")], check=True, capture_output=True)
    for path in sorted((work / "fixtures").iterdir()):
        shutil.copyfile(path, args.output / path.name)
    for mode, name in [("moves", "moves.txt"), ("cross", "cross.txt")]:
        result = subprocess.run([native, mode, str(work / mode)], check=True, capture_output=True)
        (args.output / name).write_bytes(result.stdout)
sources = [
    "lib_storage/storage/storage_encryption.cpp",
    "lib_storage/storage/storage_encryption.h",
    "lib_storage/storage/storage_encrypted_file.cpp",
    "lib_storage/storage/storage_encrypted_file.h",
    "lib_base/base/file_lock_posix.cpp",
    "lib_base/base/file_lock.h",
    "lib_base/base/random.cpp",
    "lib_base/base/bytes.cpp",
]
manifest = {
    "original": original,
    "entropy": "RAND_bytes byte counter, reset to zero for each key64/key256 file",
    "ctr_vectors": 30,
    "file_probes": len((args.output / "probes.txt").read_text().splitlines()),
    "move_probes": 8,
    "cross_filesystem": "/tmp to /dev/shm, different st_dev verified before generation",
    "source_sha256": {
        name: hashlib.sha256((args.telegram_root / name).read_bytes()).hexdigest()
        for name in sources
    },
    "fixture_sha256": {
        path.name: hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(args.output.iterdir())
        if path.is_file() and path.name not in ["README.md", "manifest.json"]
    },
}
(args.output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
print(json.dumps({key: manifest[key] for key in ["original", "ctr_vectors", "file_probes", "move_probes"]}))
