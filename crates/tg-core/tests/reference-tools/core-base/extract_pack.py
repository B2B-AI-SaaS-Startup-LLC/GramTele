from pathlib import Path
import sys
s=(Path(sys.argv[1])/'SourceFiles/lang/lang_instance.cpp').read_text()
a=s.index('QByteArray Instance::serialize() const {');b=s.index('\nvoid Instance::fillFromSerialized',a)
method=s[a:b]
pre=r'''
#include <QtCore/QString>
#include <QtCore/QByteArray>
#include <QtCore/QDataStream>
#include <QtCore/QFile>
#include <QtCore/QIODevice>
#include <map>
#include <memory>
namespace Serialize { int stringSize(const QString &s){return 4+s.size()*2;} int bytearraySize(const QByteArray &b){return 4+b.size();} }
const auto kSerializeVersionTag=QStringLiteral("#new");
constexpr auto kSerializeVersion=1;
struct Instance {
 QString _id,_pluralId,_name,_nativeName,_customFilePathAbsolute,_customFilePathRelative;
 QByteArray _customFileContent;
 int _version=0;
 std::map<QByteArray,QByteArray> _nonDefaultValues;
 std::unique_ptr<Instance> _base;
 QByteArray serialize() const;
};
'''
main=r'''
int main(int argc,char **argv){if(argc!=2)return 2;Instance p;p._id=QStringLiteral("ru");p._pluralId=QStringLiteral("ru");p._name=QStringLiteral("Russian");p._nativeName=QString::fromUtf8("Русский");p._version=42;p._nonDefaultValues["lng_language_name"]=QString::fromUtf8("Язык").toUtf8();p._nonDefaultValues["cloud_test"]=QByteArray("a\0b",3);p._base=std::make_unique<Instance>();p._base->_id=QStringLiteral("en");p._base->_pluralId=QStringLiteral("en");QFile file(argv[1]);if(!file.open(QIODevice::WriteOnly))return 3;return file.write(p.serialize())==p.serialize().size()?0:4;}
'''
Path(sys.argv[2]).write_text(pre+method+main)
