#include "codegen/emoji/data.h"
#include "codegen/emoji/options.h"
#include <QJsonDocument>
#include <QJsonArray>
#include <QFile>
QJsonArray units(const QString &s){QJsonArray a;for(auto c:s)a.append(c.unicode());return a;}
int main(int argc,char **argv){QCoreApplication app(argc,argv);auto options=codegen::emoji::parseOptions();auto data=codegen::emoji::PrepareData(options.dataPath,options.oldDataPaths);if(data.list.empty())return 1;QJsonArray map,required,replaces;
for(auto &[key,index]:data.map)map.append(QJsonArray{units(key),index});for(auto index:data.postfixRequired)required.append(index);for(auto &[key,index]:data.replaces)replaces.append(QJsonArray{key,index});
QFile file(options.outputPath+"/emoji-ir.json");if(!file.open(QIODevice::WriteOnly))return 2;file.write(QJsonDocument(QJsonArray{map,required,replaces}).toJson(QJsonDocument::Compact));}
