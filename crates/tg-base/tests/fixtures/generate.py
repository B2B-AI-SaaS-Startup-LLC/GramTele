# Run with CPython + reference zlib; fixtures are committed and tests do not
# require Python. Input is public, synthetic theme text, not user data.
import io
import pathlib
import zipfile
import zlib
root = pathlib.Path(__file__).parent
text = b"windowBg: #ffffff;\nwindowFg: #000000;\n" * 16
(root / 'theme.txt').write_bytes(text)
(root / 'theme.zlib').write_bytes(zlib.compress(text, 9))
for method, name in [(zipfile.ZIP_STORED, 'stored.zip'), (zipfile.ZIP_DEFLATED, 'deflated.zip')]:
    out = io.BytesIO()
    with zipfile.ZipFile(out, 'w', compression=method, compresslevel=9) as archive:
        info = zipfile.ZipInfo('colors.tdesktop-theme', (2026, 11, 12, 1, 2, 4))
        info.compress_type = method
        archive.writestr(info, text)
        archive.writestr('empty', b'')
    (root / name).write_bytes(out.getvalue())
print('CPython reference zlib', zlib.ZLIB_VERSION)

# Optional reference minizip shared library path as argv[1]. Uses only public
# minizip API, never builds or mutates the AyuGram source tree.
import ctypes
import sys
if len(sys.argv) > 1:
    library = ctypes.CDLL(sys.argv[1])
    library.zipOpen.argtypes = [ctypes.c_char_p, ctypes.c_int]
    library.zipOpen.restype = ctypes.c_void_p
    library.zipOpenNewFileInZip.argtypes = [ctypes.c_void_p, ctypes.c_char_p,
        ctypes.c_void_p, ctypes.c_void_p, ctypes.c_uint, ctypes.c_void_p,
        ctypes.c_uint, ctypes.c_char_p, ctypes.c_int, ctypes.c_int]
    library.zipWriteInFileInZip.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_uint]
    library.zipCloseFileInZip.argtypes = [ctypes.c_void_p]
    library.zipClose.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
    for method, name in [(0, 'minizip-stored.zip'), (8, 'minizip-deflated.zip')]:
        path = root / name
        handle = library.zipOpen(bytes(path), 0)
        assert handle
        assert library.zipOpenNewFileInZip(handle, b'a', None, None, 0, None, 0, None, method, 6) == 0
        assert library.zipWriteInFileInZip(handle, b'hello', 5) == 0
        assert library.zipCloseFileInZip(handle) == 0
        assert library.zipClose(handle, None) == 0

# Force genuine ZIP64 end records and central/local extra fields on tiny data.
zipfile.ZIP64_LIMIT = 1
out = io.BytesIO()
with zipfile.ZipFile(out, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
    for name, content in [('colors.tdesktop-theme', text), ('empty', b'')]:
        info = zipfile.ZipInfo(name, (2026, 11, 12, 1, 2, 4))
        info.compress_type = zipfile.ZIP_DEFLATED
        with archive.open(info, 'w', force_zip64=True) as file:
            file.write(content)
(root / 'zip64.zip').write_bytes(out.getvalue())
