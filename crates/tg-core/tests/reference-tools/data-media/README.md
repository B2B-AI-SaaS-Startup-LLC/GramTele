# data-media native reference (package 04)

Pinned original: `0f2a31bb420de98f331990432265b131b8d836b5`.
The original checkout is read-only. All compilation/generated files live under
`/tmp/ayugram-data-media-native`. No account, network loader or user tdata is used.
No dependency or Session/Document/Photo replacement class is supplied.

`extract.py` copies unchanged standalone function bodies from the original:
waveform encode/decode, duration rounding, attribute/transcode quality, sticker
dimensions, Core filename extension/classification and MIME predicates/wrappers,
and base FileNameFromUserString with its actual Linux platform body. Core's
actual MimeType header/class and actual QMimeDatabase are used, not a handpicked
database replacement. Real base algorithm/debug/integration/assertion sources
provide SafeRound. The filter target extracts the original pure block from
FileNameUnsafe into an argument-only wrapper; it does not execute a fake Session
or the dialog branch. QSize::scaled is the actual Qt method; PhotoData's own
method is covered by source reading and Rust tests, not executed in C++.

Reproduce from the Rust repository root in `nix develop` (provide the real Qt,
GSL and range-v3 package paths installed on the machine):

```sh
cmake -S crates/tg-core/tests/reference-tools/data-media \
  -B /tmp/ayugram-data-media-native -G Ninja \
  -DTELEGRAM_ROOT=/home/yarik/Projects/AyuGramDesktop/Telegram \
  -DQt6_DIR=/path/to/qtbase/lib/cmake/Qt6 \
  -DGSL_INCLUDE=/path/to/microsoft-gsl/include \
  -DRANGE_INCLUDE=/path/to/range-v3/include
cmake --build /tmp/ayugram-data-media-native -j2
/tmp/ayugram-data-media-native/native_media > /tmp/data-media-pure-1.csv
/tmp/ayugram-data-media-native/native_media > /tmp/data-media-pure-2.csv
cmp /tmp/data-media-pure-1.csv /tmp/data-media-pure-2.csv
```

Captured environment: Qt 6.11.2, GCC 15.3 x86_64 `-O2`, English QLocale.
The two outputs match byte for byte: **1706 rows**, **1688 compared cases +18
registry-input rows**. Rust loads those 18 actual registry responses as test
inputs, independently checks Core's overrides/alias mapping/glob order/filter
strings against native wrapper output, and compares all other rows directly.
This proves the pure wrapper, **not a production platform MIME backend**.
The actual Core registry adapter is still package 33; production has only a
required MimeDatabase interface, with no default/empty provider. PLAN.md has
not prescribed a replacement dependency, so none was added.

The corpus includes all extensions declared by native QMimeDatabase, all
platform-independent sets through Linux classification, 129 waveform encode
cases, 65 raw decode cases, 196 dimension pairs (including invalid/extreme
values), 13 duration inputs, 60 quality pairs, 18 MIME inputs, and 81 filter
promotion cases, including Unicode separators. Windows/macOS tables and Windows
save-name security rules are own tests + source reading, not native OS proof.
No assertion about a different Qt/Unicode/MIME registry version is made.

Native exposed and corrected two Rust translation mistakes before commit:
QSize with a zero side scales to the requested rectangle, and PCRE `\s` in the
filter block has ASCII semantics without UseUnicodeProperties (NBSP/em-space/
line separator do not match). These are translation fixes, not original bugs.
Waveform is explicit LE instead of C++ unaligned host uint16 accesses.

The actual original core/file_location.cpp is also linked and executed, including constructor/check/in-media-cache behavior on a temporary file with fixed modification time. Rust model fields/checks match those four native cases; platform bookmark persistence is a required service boundary, not a default stub.

Exhaustive Qt case-insensitive comparison of every Unicode scalar against ASCII letters found exactly two non-ASCII mappings: long s U+017F and Kelvin sign U+212A. Production suffix/prefix helpers preserve these, including PCRE filter promotion and filename security. Five extra filename cases and four filter cases exercise the distinction from ordinary lowercasing. This does not assert Windows/macOS filesystem/bookmark behavior.

Full-model probe of actual data_document.cpp with real includes and generated
scheme failed on generated `lang_auto.h` (see `/tmp/data-media-full-probe.log`).
No fake generated header or model body was used to bypass it. Attribute type
classification, repeated attributes, model mutations, qualities and TL metadata
are **own tests + reading**, not native class replay. Original full Session,
cache/loader callbacks, byte-lease loader cancellation, pixels, platform bookmarks and
History eligibility remain unproved. Equal-quality/equal-size unstable-sort
identity order is not proven equal between Rust and libstdc++.

The committed source SHA256 manifest covers the original files used by the
extractor. Each future corpus change must rerun native, compare two outputs and
review the proof scope; regenerating expected values from Rust is not evidence.
