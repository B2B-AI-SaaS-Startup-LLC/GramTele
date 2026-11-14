#include "pure.cpp"
#include <bit>
#include "core/file_location.h"
#include <QtCore/QTemporaryDir>
#include <QtCore/QFile>
std::string hex(const QByteArray &s) { return s.toHex().toStdString(); }
std::string hex(const QString &s) { return hex(s.toUtf8()); }
int main(int argc, char **argv) {
 QCoreApplication application(argc, argv);
 QLocale::setDefault(QLocale::English);
 for (int n = 0; n <= 128; ++n) {
  auto wave = VoiceWaveform();
  for (int i = 0; i < n; ++i) wave.push_back(static_cast<signed char>((i*37+n) & 255));
  auto bytes = documentWaveformEncode5bit(wave);
  auto decoded = documentWaveformDecode(bytes);
  auto raw = QByteArray(); for (auto v : wave) raw.push_back(char(v));
  auto back = QByteArray(); for (auto v : decoded) back.push_back(char(v));
  std::cout << "wave," << hex(raw) << ',' << hex(bytes) << ',' << hex(back) << '\n';
 }
 for (int n = 0; n <= 64; ++n) {
  auto bytes = QByteArray(); for (int i = 0; i < n; ++i) bytes.push_back(char((i*71+n)&255));
  auto decoded = documentWaveformDecode(bytes); auto raw = QByteArray(); for (auto v : decoded) raw.push_back(char(v));
  std::cout << "decode," << hex(bytes) << ',' << hex(raw) << '\n';
 }
 const int sizes[] = {-1,0,1,2,3,7,100,512,2559,2560,2561,4000,100000,INT_MAX};
 for (int w : sizes) for (int h : sizes) {
  auto scaled = QSize(w,h).scaled(2560,2560,Qt::KeepAspectRatio);
  std::cout << "size," << w << ',' << h << ',' << scaled.width() << ',' << scaled.height() << ',' << GoodStickerDimensions(w,h) << '\n';
 }
 const double seconds[] = {-1e30,-9223372036854776.,-1.0005,-0.0005,0.,0.0005,0.0015,1.0005,1e6,9223372036854776.,1e30,std::numeric_limits<double>::infinity(),std::numeric_limits<double>::quiet_NaN()};
 for (auto s : seconds) std::cout << "ms," << std::bit_cast<uint64_t>(s) << ',' << MillisecondsFromSeconds(s) << '\n';
 for (int a : {-1,0,1,99,100,149,150,151,1000,INT_MAX}) for (int m : {-1,0,1,100,1000,INT_MAX}) std::cout << "quality," << a << ',' << m << ',' << ResolveAttributeVsTranscodeQuality(a,m) << '\n';
 auto names = QStringList{QString(),u"x.JPG"_q,u"dir.png/file"_q,u"dir/a.exe.JPG"_q,u".png"_q,u"x.a/b"_q,u"x\\a.opus"_q,u"CON.lnk"_q,u"NUL.jpg"_q,u" a. "_q,u"a\u202eb\u202cc\u2068d"_q,u"a<>:\"|?*/\\b"_q};
 // Every declared extension of the actual QMimeDatabase plus unusual examples.
 for (const auto &mime : QMimeDatabase().allMimeTypes()) for (auto glob : mime.globPatterns()) if (glob.startsWith("*.")) names.push_back("file" + glob.mid(1));
 names.removeDuplicates(); std::sort(names.begin(), names.end());
 names.append({u"file.G\u0130F"_q,u"file.opu\u017f"_q,u"file.\u017fvg"_q,u"file.LN\u212a"_q,u"file.\u0130co"_q});
 for (const auto &name : names) std::cout << "name," << hex(name) << ',' << hex(Core::FileExtension(name)) << ',' << int(Core::DetectNameType(name)) << ',' << Core::NameTypeAllowsThumbnail(Core::DetectNameType(name)) << ',' << hex(base::FileNameFromUserString(name)) << '\n';
 const auto mimes = QStringList{u"image/webp"_q,u"application/x-tgsticker"_q,u"application/x-tgwallpattern"_q,u"application/x-tdesktop-theme"_q,u"application/x-tgtheme-tdesktop"_q,u"application/x-tdesktop-palette"_q,u"audio/mpeg3"_q,u"audio/mp3"_q,u"audio/wav"_q,u"audio/ogg"_q,u"video/mp4"_q,u"video/webm"_q,u"video/quicktime"_q,u"image/jpeg"_q,u"image/png"_q,u"image/gif"_q,u"IMAGE/PNG"_q,u"x/unknown"_q};
 for (const auto &m : mimes) {
  auto raw = QMimeDatabase().mimeTypeForName(m == u"audio/mpeg3"_q ? u"audio/mp3"_q : m);
  std::cout << "registry," << hex(m == u"audio/mpeg3"_q ? u"audio/mp3"_q : m) << ',' << hex(raw.name()) << ',' << hex(raw.globPatterns().join('|')) << ',' << hex(raw.filterString()) << '\n';
  auto r = Core::MimeTypeForName(m);
  std::cout << "mime," << hex(m) << ',' << hex(r.name()) << ',' << hex(r.globPatterns().join('|')) << ',' << hex(r.filterString()) << ',' << Core::IsMimeStickerLottie(m) << Core::IsMimeStickerWebm(m) << Core::IsMimeStickerAnimated(m) << Core::IsMimeSticker(m) << Core::IsMimeSentAsVideo(m) << Core::IsMimeAcceptedForPhotoVideoAlbum(m) << '\n';
 }
 const auto filterNames = QStringList{u"x"_q,u"x.ogg"_q,u"x.MP3"_q,u"x.wav"_q,u"x.tdesktop-theme"_q,u"x.a_9"_q,u"x.a\u0430"_q};
 const auto filters = QStringList{u""_q,u"Audio (*.mp3 *.ogg);;All (*)"_q,u"Audio (*.MP3 *.ogg);;All (*)"_q,u"Audio (*.mp3 *.ogg *.wav);;All (*)"_q,u"Audio (*.mp3 *.wav);;All (*);;"_q,u"Audio (*.wav)"_q,u"Any;;All (*)"_q,u"Foo (*.x *.a_9);;All (*)"_q,u"Audio (*.mp3\u00a0*.ogg);;All (*)"_q,u"Audio (*.mp3\u2003*.ogg);;All (*)"_q,u"Audio (*.mp3\u2028*.ogg);;All (*)"_q};
 for (const auto &n : filterNames) for (const auto &f : filters) std::cout << "filter," << hex(n) << ',' << hex(f) << ',' << hex(NativePromoteFilter(n,f)) << '\n';
 for (const auto &n : QStringList{u"x.ok"_q,u"x.mps"_q}) for (const auto &f:QStringList{u"Files (*.o\u212a *.mp\u017f);;All (*)"_q,u"Files (*.foo *.o\u212a *.mp\u017f);;All (*)"_q}) std::cout<<"filter,"<<hex(n)<<','<<hex(f)<<','<<hex(NativePromoteFilter(n,f))<<'\n';
 auto printLocation = [](const char *name, const Core::FileLocation &location) {
  std::cout << "fileloc," << name << ',' << location.isEmpty() << ',' << location.inMediaCache() << ',' << location.size << ',' << location.modified.toMSecsSinceEpoch() << ',' << location.check() << '\n';
 };
 printLocation("empty", Core::FileLocation());
 printLocation("cache", Core::FileLocation::InMediaCacheLocation());
 QTemporaryDir dir;
 QFile file(dir.path() + "/file.bin");
 if (!file.open(QIODevice::ReadWrite) || file.write("abc", 3) != 3 || !file.flush() || !file.setFileTime(QDateTime::fromMSecsSinceEpoch(1600000000123,Qt::UTC),QFileDevice::FileModificationTime)) return 3;
 file.close();
 Core::FileLocation location(file.fileName());
 printLocation("file", location);
 if (!file.open(QIODevice::Append) || file.write("d",1)!=1) return 4;
 file.close();
 printLocation("changed", location);
 // Exhaustive single-scalar Qt case-insensitive matches against ASCII letters.
 // Only non-ASCII matches are emitted; ASCII uses ordinary ASCII lower case.
 for (char32_t code=128;code<=0x10ffff;++code) {
  if(code>=0xd800&&code<=0xdfff)continue;
  const auto text=QString::fromUcs4(&code,1);
  for(char a='a';a<='z';++a) if(text.compare(QString(QChar(a)),Qt::CaseInsensitive)==0) std::cout<<"asciifold,"<<uint32_t(code)<<','<<a<<'\n';
 }
 for (const auto &name:QStringList{u"x.LNK "_q,u"x.\u017fcf"_q,u"x.ln\u212a"_q,u"x.scf"_q,u"x.exe"_q,QString(),u"x.scf."_q,u"x.lnk\u0085"_q,u"x.lnk\u001c"_q,u"x.lnk\u00a0"_q,u"x.lnk\u202f"_q}) std::cout<<"savedmask,"<<hex(name)<<','<<hex(NativeSavedNameMask(name))<<'\n';
}
