"""Generate deterministic operations and run the unmodified native list models."""
import argparse
import hashlib
import json
import random
import subprocess
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("native", type=Path)
parser.add_argument("output", type=Path)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
rng = random.Random(0xA7_2026_11_13)
maximum = 1 << 56
operations = []


def emit(*parts):
    operations.append(" ".join(map(str, parts)))


def sequence():
    lower, upper = sorted(rng.sample([0, 1, 5, 10, 20, 40, 100, 1 << 40, maximum], 2))
    values = [rng.randint(lower, upper) for _ in range(rng.randrange(6))]
    if values and rng.randrange(3) == 0:
        values.append(values[0])
    return lower, upper, values


emit("sq", 1, 0, 0)
emit("sc", 0, 10, 2)
emit("ss", 0, 0, -1, 0)
emit("sq", 0, 0, 0)
emit("ss", 0, 20, 10, 2, 10, 20)
emit("sq", 10, 0, 0)
emit("ss", 40, maximum, -1, 2, 40, 50)
emit("sq", 40, 0, 0)
emit("se", 30, 20, 40)
emit("sc", 20, 2, 2)
emit("sc", 20, 3, 2)
emit("si")
emit("ss", 20, 2, 9, 1, 7)
emit("ss", 30, 30, 0, 0)
emit("sq", 0, 5, 5)

for _ in range(400):
    command = rng.choice(["ss", "ss", "sn", "sr", "se", "sa", "si"])
    lower, upper, values = sequence()
    if command == "ss":
        emit(command, lower, upper, rng.choice([-1, 0, 1, 10, 100]), len(values), *values)
    elif command == "se":
        emit(command, rng.randint(lower, upper), lower, upper)
    elif command in ["sn", "sr"]:
        emit(command, rng.choice([1, 5, 10, 30, 100, 1 << 40, maximum - 1]))
    else:
        emit(command)
    for around in [0, 10, 25, 1 << 40, maximum - 1]:
        emit("sq", around, rng.randrange(5), rng.randrange(5))
    emit("sc", rng.choice([0, 10, 30, maximum - 1]), rng.choice([-1, 0, 2, 50]), 3)

for _ in range(350):
    user = rng.choice([1, 2, (1 << 63) + 9])
    photo = rng.choice([0, 1, 2, 3, 9, 1 << 40, (1 << 64) - 1])
    command = rng.choice(["pn", "pb", "ps", "pr", "pa", "px"])
    if command == "ps":
        values = [rng.randrange(1, 6) for _ in range(rng.randrange(6))]
        emit(command, user, rng.choice([-1, 0, 1, 5, 20]), len(values), *values)
    elif command == "px":
        emit(command, user, photo, rng.randrange(10))
    else:
        emit(command, user, photo)
    for id_ in [0, photo, 2, 99]:
        emit("pq", user, id_, rng.randrange(2), rng.randrange(4), rng.randrange(4))
emit("pq", 999, 1, 0, 0, 0)

# Establish independent peer, topic and monoforum entries before propagation.
scopes = [(1, 0, 0), (1, 2, 0), (1, 0, 3), (1, 2, 3), (1, 5, 0), (2, 0, 0)]
for peer, topic, mono in scopes:
    emit("me", peer, topic, mono, 8191, 10, 0, 20)
emit("mn", 1, 2, 3, 8191, 30)
emit("mr", 1, 2, 3, 5, 10)
emit("ma", 1, 0, 3, 8191)
for scope in scopes:
    emit("mq", *scope, 0, 10, 5, 5)

for _ in range(350):
    peer, topic, mono = rng.choice(scopes)
    bits = rng.choice([0, 1, 3, 1 << 12, 8191, (1 << 32) - 1])
    id_ = rng.choice([1, 5, 10, 20, 30, 1 << 40])
    type_ = rng.randrange(13)
    command = rng.choice(["ms", "mn", "me", "mr", "ma", "mi", "mu"])
    if command == "ms":
        lower, upper, values = sequence()
        emit(command, peer, topic, mono, type_, lower, upper,
             rng.choice([-1, 0, 1, 10, 100]), len(values), *values)
    elif command in ["mn", "mr"]:
        emit(command, peer, topic, mono, bits, id_)
    elif command == "me":
        emit(command, peer, topic, mono, bits, id_, 0, maximum)
    elif command == "ma":
        emit(command, peer, topic, mono, bits)
    elif command == "mu":
        emit(command, peer, topic, mono)
    else:
        emit(command, peer)
    for scope in scopes:
        emit("mq", *scope, type_, id_, rng.randrange(4), rng.randrange(4))

payload = ("\n".join(operations) + "\n").encode()
result = subprocess.run([str(args.native.resolve())], input=payload, capture_output=True, check=True)
for name, data in [("storage-trace.txt", payload), ("storage-trace-native.txt", result.stdout)]:
    (args.output / name).write_bytes(data)
manifest = {
    "original": "0f2a31bb420de98f331990432265b131b8d836b5",
    "seed": "0xA720261113",
    "commands": len(operations),
    "sha256": {
        name: hashlib.sha256((args.output / name).read_bytes()).hexdigest()
        for name in ["storage-trace.txt", "storage-trace-native.txt"]
    },
}
(args.output / "storage-trace-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
print(json.dumps(manifest))
