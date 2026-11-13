"""Extract unchanged generated Peer functions; the full original headers remain in use."""
import re
import sys
from pathlib import Path

source = Path(sys.argv[1]).read_text()
methods = []
for name in ["User", "Chat", "Channel"]:
    match = re.search(r"inline static MTPpeer new_peer" + name + r"\([^\n]*\) \{", source)
    if not match:
        raise SystemExit(f"Missing original creator peer{name}")
    depth, end = 1, match.end()
    while depth:
        depth += (source[end] == "{") - (source[end] == "}")
        end += 1
    methods.append(source[match.start():end])
start = source.index("MTPpeer::MTPpeer() = default;")
end = source.index("MTPstorage_fileType::MTPstorage_fileType() = default;", start)
output = '#include "scheme.h"\nnamespace MTP::details {\nclass TypeCreator final {\npublic:\n'
output += "\n".join(methods) + "\n};\n}\n" + source[start:end]
Path(sys.argv[2]).write_text(output)
