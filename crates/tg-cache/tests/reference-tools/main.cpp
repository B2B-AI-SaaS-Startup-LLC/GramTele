#include "storage/storage_encryption.h"
#include "storage/storage_encrypted_file.h"
#include "base/openssl_help.h"
#include <array>
#include <QtCore/QTemporaryDir>

unsigned entropy = 0;
QByteArray forcedSalt;
extern "C" int __wrap_RAND_bytes(unsigned char *out, int count) {
    for (auto i = 0; i < count; ++i) {
        out[i] = forcedSalt.isEmpty() ? entropy++ : static_cast<unsigned char>(forcedSalt.at(i));
    }
    forcedSalt.clear();
    return 1;
}
void require(bool value, const char *what) {
    if (!value) { std::cerr << what << '\n'; std::exit(1); }
}
Storage::EncryptionKey key(int size) {
    auto data = bytes::vector(size);
    for (auto i = 0; i < size; ++i) data[i] = static_cast<bytes::type>(i);
    return Storage::EncryptionKey(std::move(data));
}
bytes::vector pattern(int size, int start) {
    auto data = bytes::vector(size);
    for (auto i = 0; i < size; ++i) data[i] = static_cast<bytes::type>(start + i);
    return data;
}
QByteArray readAll(const QString &path) {
    QFile file(path); require(file.open(QIODevice::ReadOnly), "readAll open");
    return file.readAll();
}
void save(const QString &path, const QByteArray &data) {
    QFile file(path); require(file.open(QIODevice::WriteOnly), "save open");
    require(file.write(data) == data.size(), "save write");
}
QByteArray hex(bytes::const_span data) {
    return QByteArray(reinterpret_cast<const char*>(data.data()), data.size()).toHex();
}
void writeFlow(const QString &path, int size) {
    Storage::File file;
    require(file.open(path, Storage::File::Mode::Write, key(size)) == Storage::File::Result::Success, "flow open");
    auto data = pattern(32, 0); require(file.write(data), "flow first");
    data = pattern(19, 0x80); require(file.writeWithPadding(data), "flow padding");
    require(file.seek(16), "flow seek");
    data = pattern(16, 0x40); require(file.write(data) && file.flush(), "flow overwrite");
    file.close();
    require(file.open(path, Storage::File::Mode::ReadAppend, key(size)) == Storage::File::Result::Success, "flow append open");
    require(file.seek(file.size()), "flow append seek");
    data = pattern(16, 0xE0); require(file.write(data) && file.flush(), "flow append");
}
void probe(QTextStream &out, const QString &root, const QString &name, int keySize) {
    Storage::File file;
    const auto result = file.open(root + '/' + name, Storage::File::Mode::Read, key(keySize));
    auto data = bytes::vector(32);
    const auto count = result == Storage::File::Result::Success ? file.read(data) : 0;
    out << name << ' ' << keySize << ' ' << int(result) << ' ' << file.isOpen()
        << ' ' << file.size() << ' ' << file.offset() << ' ' << count << ' ' << hex(data) << '\n';
}
QByteArray alteredHeader(QByteArray file, bool unknownFormat) {
    auto state = key(64).prepareCtrState(bytes::make_span(file).subspan(4,64));
    auto body = bytes::make_detached_span(file).subspan(68,48);
    state.decrypt(body,0);
    if (unknownFormat) body[0] = static_cast<bytes::type>(1);
    else { body[1] = static_cast<bytes::type>(0xFF); body[8] = static_cast<bytes::type>(9); }
    const auto checksum = openssl::Sha256(key(64).data(), bytes::make_span(file).subspan(4,80));
    bytes::copy(body.subspan(16),checksum);
    state.encrypt(body,0);
    return file;
}
void fixtures(const QString &root) {
    require(!QFileInfo::exists(root) && QDir().mkpath(root), "fixtures output must be new");
    for (auto size : {64,256}) {
        entropy = 0;
        writeFlow(root + QString("/key%1.tdef").arg(size), size);
    }
    QFile vectors(root + "/ctr.bin"); require(vectors.open(QIODevice::WriteOnly), "vectors open");
    QDataStream stream(&vectors); stream.setByteOrder(QDataStream::LittleEndian);
    const std::array<qint64,6> offsets = {0,16,256,4096,1LL<<40,(std::numeric_limits<qint64>::max()/16)*16};
    const auto record = [&](bytes::const_span dataKey, bytes::const_span saltOrIv, bool derived, qint64 offset) {
        auto plain = pattern(96,0x31);
        auto encrypted = plain;
        auto state = derived ? Storage::EncryptionKey(bytes::vector(dataKey.begin(),dataKey.end())).prepareCtrState(saltOrIv)
            : Storage::CtrState(dataKey,saltOrIv);
        state.encrypt(encrypted,offset);
        stream << quint32(derived) << quint32(dataKey.size());
        stream.writeRawData(reinterpret_cast<const char*>(dataKey.data()),dataKey.size());
        stream << quint32(saltOrIv.size());
        stream.writeRawData(reinterpret_cast<const char*>(saltOrIv.data()),saltOrIv.size());
        stream << offset << quint32(plain.size());
        stream.writeRawData(reinterpret_cast<const char*>(plain.data()),plain.size());
        stream.writeRawData(reinterpret_cast<const char*>(encrypted.data()),encrypted.size());
    };
    stream << quint32(30);
    for (auto variant = 0; variant != 3; ++variant) {
        auto iv = pattern(16,variant == 0 ? 0 : 0xF0);
        if (variant == 0) std::fill(iv.begin(),iv.end(),static_cast<bytes::type>(0));
        if (variant == 2) std::fill(iv.begin(),iv.end(),static_cast<bytes::type>(0xFF));
        for (auto offset : offsets) record(pattern(32,0x10),iv,false,offset);
    }
    for (auto size : {64,256}) for (auto offset : offsets) record(key(size).data(),pattern(64,0),true,offset);
    require(stream.status() == QDataStream::Ok, "vectors write");
    QFile probes(root + "/probes.txt"); require(probes.open(QIODevice::WriteOnly), "probes open");
    QTextStream out(&probes);
    for (auto size : {64,256}) probe(out,root,QString("key%1.tdef").arg(size),size);
    probe(out,root,"key64.tdef",256);
    const auto original = readAll(root + "/key64.tdef");
    for (auto length : {0,3,4,16,64,67,68,115,116,117,131,132,191}) {
        auto name = QString("truncated%1.tdef").arg(length);
        save(root + '/' + name,original.first(length)); probe(out,root,name,64);
    }
    for (auto position : {0,4,68,116}) {
        auto file = original; file[position] = file.at(position) ^ 0x80;
        auto name = QString("flipped%1.tdef").arg(position);
        save(root + '/' + name,file); probe(out,root,name,64);
    }
    for (auto unknown : {false,true}) {
        auto name = unknown ? QString("unknown-format.tdef") : QString("reserved.tdef");
        save(root + '/' + name,alteredHeader(original,unknown)); probe(out,root,name,64);
    }
    std::cout << "generated " << root.toStdString() << '\n';
}
void verify(const QString &root) {
    for (auto size : {64,256}) {
        const auto path = root + QString("/key%1.tdef").arg(size);
        Storage::File reader;
        require(reader.open(path,Storage::File::Mode::Read,key(size)) == Storage::File::Result::Success, "verify open");
        require(reader.size() == 80, "verify size");
        auto plain = bytes::vector(80);
        require(reader.read(plain) == 80, "verify read");
        const auto check = [&](int offset, const auto &expected) {
            require(std::equal(expected.begin(),expected.end(),plain.begin()+offset), "verify plaintext");
        };
        check(0,pattern(16,0)); check(16,pattern(16,0x40)); check(32,pattern(19,0x80)); check(64,pattern(16,0xE0));
        reader.close();
        const auto raw = readAll(path);
        forcedSalt = raw.mid(4,64);
        const auto rewritten = path + ".native";
        require(!QFileInfo::exists(rewritten), "verify output exists");
        Storage::File writer;
        require(writer.open(rewritten,Storage::File::Mode::Write,key(size)) == Storage::File::Result::Success, "verify rewrite open");
        require(writer.write(plain) && writer.flush(), "verify rewrite");
        writer.close();
        require(raw == readAll(rewritten), "Rust and native file bytes differ");
        std::cout << "Rust/native key" << size << " read and exact rewrite PASS\n";
    }
}
void moves(const QString &root) {
    require(!QFileInfo::exists(root) && QDir().mkpath(root), "moves output must be new");
    for (auto kind = 0; kind != 8; ++kind) {
        const auto dir = root + QString("/%1").arg(kind); require(QDir().mkpath(dir), "move case dir");
        const auto source = dir + "/from";
        const auto target = kind == 3 ? dir + "/missing/to" : kind == 4 ? source : dir + "/to";
        const auto aux = dir + "/aux";
        if (kind == 5) { require(QDir().mkpath(source), "source dir"); save(source + "/child","source"); }
        else if (kind == 7) { save(aux,"source"); require(QFile::link(aux,source), "source symlink"); }
        else if (kind != 2) save(source,"source");
        if (kind == 1) save(target,"target");
        else if (kind == 6) { save(aux,"target"); require(QFile::link(aux,target), "destination symlink"); }
        const auto result = Storage::File::Move(source,target);
        const auto info = QFileInfo(target);
        const auto content = info.isFile() ? readAll(target).toHex() : QByteArray("-");
        const auto auxContent = QFileInfo::exists(aux) ? readAll(aux).toHex() : QByteArray("-");
        std::cout << kind << ' ' << result << ' ' << QFileInfo::exists(source) << ' '
            << info.exists() << ' ' << info.isDir() << ' ' << info.isSymLink() << ' '
            << content.toStdString() << ' ' << (auxContent.isEmpty() ? "empty" : auxContent.toStdString()) << '\n';
    }
}
void cross(const QString &root) {
    require(!QFileInfo::exists(root) && QDir().mkpath(root), "cross output must be new");
    QTemporaryDir other("/dev/shm/ayu-cache-native-cross-XXXXXX");
    require(other.isValid(), "cross tmpfs directory");
    const auto source = root + "/from";
    const auto target = other.path() + "/to";
    save(source,"source");
    const auto result = Storage::File::Move(source,target);
    std::cout << "cross " << result << ' ' << QFileInfo::exists(source) << ' '
        << QFileInfo::exists(target) << ' ' << readAll(target).toHex().toStdString() << '\n';
}
int main(int argc, char **argv) {
    require(argc == 3, "usage: native_cache_file fixtures|verify|moves|cross output");
    const auto mode = std::string(argv[1]);
    if (mode == "fixtures") fixtures(QString::fromLocal8Bit(argv[2]));
    else if (mode == "verify") verify(QString::fromLocal8Bit(argv[2]));
    else if (mode == "moves") moves(QString::fromLocal8Bit(argv[2]));
    else if (mode == "cross") cross(QString::fromLocal8Bit(argv[2]));
    else { std::cerr << "invalid mode\n"; return 1; }
}
