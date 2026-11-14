"""Unchanged standalone original bodies; no replacement model/Session classes."""
from pathlib import Path
import hashlib, json, sys
root, out = map(Path, sys.argv[1:])
def read(name):
    return (root / 'SourceFiles' / name).read_text()
def function(source, signature):
    start = source.index(signature)
    return source[start:source.index('\n}', start) + 2] + '\n'
mime = read('core/mime_type.cpp')
doc = read('data/data_document.cpp')
text = '#include "core/mime_type.h"\n#include "media/media_common.h"\n'
text += 'namespace Core {\n'
for signature in ['[[nodiscard]] base::flat_set<QString> SplitExtensions(',
                  'MimeType::MimeType(const QMimeType', 'MimeType::MimeType(Known',
                  'QStringList MimeType::globPatterns(', 'QString MimeType::filterString(',
                  'QString MimeType::name(', 'MimeType MimeTypeForName(',
                  'QString FileExtension(', 'NameType DetectNameType(',
                  'bool NameTypeAllowsThumbnail(', 'bool FileIsImage(',
                  'bool IsMimeStickerLottie(', 'bool IsMimeStickerWebm(',
                  'bool IsMimeStickerAnimated(', 'bool IsMimeSticker(',
                  'bool IsMimeSentAsVideo(', 'bool IsMimeAcceptedForPhotoVideoAlbum(']:
    text += function(mime, signature)
text += '}\n'
text += 'using VoiceWaveform = QVector<signed char>;\n'
for signature in ['VoiceWaveform documentWaveformDecode(',
                  'QByteArray documentWaveformEncode5bit(',
                  '[[nodiscard]] crl::time MillisecondsFromSeconds(',
                  '[[nodiscard]] int ResolveAttributeVsTranscodeQuality(']:
    text += function(doc, signature)
text += function(read('data/data_types.cpp'), 'bool GoodStickerDimensions(')
text += function(doc, 'QString JoinStringList(')
start = doc.index('QString ext = QFileInfo(name).suffix()')
end = doc.index('\n\t\treturn filedialogGetSaveFile', start)
text += 'QString NativePromoteFilter(const QString &name, const QString &filter) {\n' + doc[start:end] + '\nreturn fil;\n}\n'
start = doc.index('const auto lower = result.trimmed().toLower()', doc.index('QString FileNameForSave('))
end = doc.index('#endif', start)
text += 'QString NativeSavedNameMask(const QString &result) {\n' + doc[start:end] + '\nreturn result;\n}\n'
text += 'namespace Platform {\n'
text += function((root/'lib_base/base/platform/linux/base_file_utilities_linux.cpp').read_text(), 'QString FileNameFromUserString(')
text += '}\nnamespace base { namespace Platform { using ::Platform::FileNameFromUserString; }\n'
text += function((root/'lib_base/base/base_file_utilities.cpp').read_text(), 'QString FileNameFromUserString(')
text += '}\n'
(out/'pure.cpp').write_text(text)
manifest = {}
for name in ['core/mime_type.cpp','core/mime_type.h','core/file_location.cpp','core/file_location.h','platform/platform_file_bookmark.h','data/data_document.cpp','data/data_types.cpp','media/media_common.h']:
    manifest[name] = hashlib.sha256((root/'SourceFiles'/name).read_bytes()).hexdigest()
for name in ['lib_base/base/base_file_utilities.cpp', 'lib_base/base/platform/linux/base_file_utilities_linux.cpp', 'lib_base/base/algorithm.cpp', 'lib_base/base/debug_log.cpp', 'lib_base/base/assertion.cpp', 'lib_base/base/integration.cpp']:
    manifest[name] = hashlib.sha256((root/name).read_bytes()).hexdigest()
(out/'source-sha256.json').write_text(json.dumps(manifest, indent=2)+'\n')
