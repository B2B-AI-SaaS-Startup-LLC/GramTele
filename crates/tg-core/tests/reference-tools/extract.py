"""Read-only extraction of original storage format functions, with an explicit OS/owner harness."""
from pathlib import Path
import sys
root=Path(sys.argv[1]);out=Path(sys.argv[2]);out.mkdir(parents=True,exist_ok=True)
s=(root/'SourceFiles/storage/details/storage_file_utilities.cpp').read_text()
h=(root/'SourceFiles/storage/details/storage_file_utilities.h').read_text().replace('#include "storage/storage_account.h"','#include "mtproto/mtproto_auth_key.h"\n#include "core/utils.h"\nnamespace Storage { using FileKey = uint64; }')
(out/'utilities.h').write_text(h)
# Whole non-scheduling section, including the original ReadFile and writer.
body=s[s.index('QString ToFilePart(FileKey val)'):s.index('\nvoid Sync()')]
utils=(root/'SourceFiles/core/utils.cpp').read_text();md5=utils[utils.index('namespace {\n\n\tinline void _md5_decode'):utils.index('\nchar *hashMd5Hex(')]
pre='''#include "utilities.h"
#include "base/openssl_help.h"
#include "base/random.h"
#include <QtCore/QDir>
#include <QtCore/QFileInfo>
#include <QtCore/QSaveFile>
#include <QtCore/QtEndian>
#include <QtCore/QDateTime>
#include <QtCore/QCryptographicHash>
#include <openssl/sha.h>
namespace Logs {bool DebugEnabled(){return false;}}
constexpr int AppVersion=7002009;
constexpr int LocalEncryptNoPwdIterCount=4,LocalEncryptIterCount=4000;
static unsigned char entropy=0;
void reset_entropy(){entropy=0;}
extern "C" int __wrap_RAND_bytes(unsigned char*p,int n){for(int i=0;i<n;++i)p[i]=entropy++;return 1;}
int32 *hashSha1(const void*p,uint32 n,void*out){SHA1((const unsigned char*)p,n,(unsigned char*)out);return (int32*)out;}
'''
manager='''
namespace base::Platform {void FlushFileData(QFile&){std::abort();}bool RenameWithOverwrite(const QString&,const QString&){std::abort();}}
namespace Storage::details {
constexpr char TdfMagic[]={'T','D','F','$'};constexpr int TdfMagicLen=4,kStrongIterationsCount=100000;
struct WriteEntry {QString basePath,base;QByteArray data,md5;};
struct WriteManager {
 void writeNow(WriteEntry&&);
 bool writeHeader(const QString&,QFileDevice&);
 template<typename File>bool open(File&,const WriteEntry&,char);
 QString path(const WriteEntry&,char)const;
 void writeSync(WriteEntry&&e){writeNow(std::move(e));}
 void write(WriteEntry&&e){writeNow(std::move(e));}
} Manager;
'''
for start,end in [('QString WriteManager::path(', '\ntemplate <typename File>'),('template <typename File>\nbool WriteManager::open(', '\nvoid WriteManager::scheduleWrite('),('void WriteManager::writeNow(', '\nvoid WriteManager::writeSyncAll('),('bool WriteManager::writeHeader(', '\nQString WriteManager::path(')]:
 manager+=s[s.index(start):s.index(end)]+'\n'
(out/'utilities.cpp').write_text(pre+md5+manager+body+'\n}\n')
# Main authorization methods are compiled unchanged with a small owner carrying their fields.
s=(root/'SourceFiles/main/main_account.cpp').read_text()
def between(start,end):return s[s.index(start):s.index(end)]
methods=between('QByteArray Account::serializeMtpAuthorization() const {','\nvoid Account::setSessionUserId(')+between('void Account::setMtpAuthorization(const QByteArray &serialized) {','\nvoid Account::startMtp(')
(out/'authorization.inc').write_text(methods)
s=(root/'SourceFiles/storage/storage_domain.cpp').read_text()
# Original key wrapping, list writer, parser. Main::Account facade opens original files below.
parts=[]
for start,end in [('void Domain::encryptLocalKey(', '\nDomain::StartModernResult Domain::startModern('),('Domain::StartModernResult Domain::startModern(', '\nvoid Domain::writeAccounts('),('void Domain::writeAccounts()', '\nvoid Domain::startFromScratch(')]:
 parts.append(s[s.index(start):s.index(end)])
(out/'domain.inc').write_text('\n'.join(parts))

(out/"domain_header.h").write_text((root/"SourceFiles/storage/storage_domain.h").read_text().replace("private:","public:"))
