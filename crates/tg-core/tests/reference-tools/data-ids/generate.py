"""Generate synthetic ID operations and capture the original C++ results."""
import argparse
import hashlib
import json
import random
import subprocess
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("native", type=Path)
parser.add_argument("output", type=Path)
parser.add_argument("--telegram-root", type=Path, required=True)
parser.add_argument("--scheme-include", type=Path, required=True)
args = parser.parse_args()
original = "0f2a31bb420de98f331990432265b131b8d836b5"
actual = subprocess.check_output(["git", "-C", str(args.telegram_root), "rev-parse", "HEAD"], text=True).strip()
if actual != original:
    raise SystemExit(f"Expected original {original}, got {actual}")
rng = random.Random(0xA7_1D_2026)
native = str(args.native.resolve())
constants = subprocess.run([native], input=b"C\n", check=True, capture_output=True).stdout.decode().split()
assert constants[0] == "C"
constant_values = list(map(int, constants[1:]))
commands = ["C"]


def emit(*values):
    commands.append(" ".join(map(str, values)))


interesting = [0, 1, 2, (1 << 31) - 1, 1 << 31, (1 << 32) - 1, 1 << 32,
               (1 << 40) + 3, (1 << 48) - 1, 1 << 48, 2 << 48, 127 << 48,
               128 << 48, 255 << 48, 1 << 55, (1 << 55) - 1, 1 << 56,
               (1 << 63) - 1, 1 << 63, (1 << 64) - 1]
for kind in range(11):
    for value in interesting:
        emit("P", kind, value)
for tag in range(16):
    for value in [0, 1, (1 << 31) - 1, 1 << 31, (1 << 32) - 1]:
        emit("P", 5, (tag << 32) | value)
        emit("P", 5, (0xA5 << 56) | (tag << 32) | value)
for tag in range(256):
    emit("P", 0, (tag << 48) | 0x123456789ABC)
for _ in range(600):
    emit("P", rng.randrange(11), rng.getrandbits(64))

minimum, maximum = -(1 << 63), (1 << 63) - 1
message_values = {minimum, maximum, -1, 0, 1, 1 << 40}
for value in constant_values:
    for offset in [-1, 0, 1]:
        if minimum <= value + offset <= maximum:
            message_values.add(value + offset)
for index, value in enumerate(sorted(message_values)):
    emit("M", value, interesting[index % len(interesting)],
         rng.choice([-(1 << 31), -1, 0, 1, (1 << 31) - 1]),
         rng.choice([0, 1, (1 << 64) - 1]), rng.choice([minimum, maximum, -1, 0, 1]))
for message in [0, -1, 1]:
    for peer in [0, 1, 1 << 48, (1 << 64) - 1]:
        for story in [0, -1, 1]:
            for session in [0, 1, (1 << 64) - 1]:
                emit("M", message, peer, story, session, message)
for _ in range(400):
    bare = rng.randrange(minimum, maximum + 1)
    emit("M", bare, rng.getrandbits(64), rng.randrange(-(1 << 31), 1 << 31),
         rng.getrandbits(64), rng.randrange(minimum, maximum + 1))
for index in [0, 1, 2, (1 << 30) - 1, 1 << 30, (1 << 31) - 1]:
    emit("I", index)

payload = ("\n".join(commands) + "\n").encode()
result = subprocess.run([native], input=payload, capture_output=True, check=True)
assert len(result.stdout.splitlines()) == len(commands)
args.output.mkdir(parents=True, exist_ok=True)
for name, data in [("cases.txt", payload), ("native.txt", result.stdout)]:
    (args.output / name).write_bytes(data)
sources = ["SourceFiles/data/data_peer_id.h", "SourceFiles/data/data_peer_id.cpp", "SourceFiles/data/data_msg_id.h"]
manifest = {
    "original": original,
    "seed": "0xA71D2026",
    "commands": len(commands),
    "by_command": {name: sum(line.startswith(name) for line in commands) for name in ["C", "P", "M", "I"]},
    "source_sha256": {name: hashlib.sha256((args.telegram_root / name).read_bytes()).hexdigest() for name in sources},
    "generated_scheme_sha256": {name: hashlib.sha256((args.scheme_include / name).read_bytes()).hexdigest() for name in ["scheme.h", "scheme.cpp"]},
    "fixture_sha256": {name: hashlib.sha256((args.output / name).read_bytes()).hexdigest() for name in ["cases.txt", "native.txt"]},
}
(args.output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
print(json.dumps({name: manifest[name] for name in ["commands", "by_command"]}))
