#include "codegen/style/parsed_file.h"
#include <QJsonArray>
#include <QJsonDocument>
#include <QFile>
using namespace codegen::style;
using namespace codegen::style::structure;
QJsonArray seq(const FullName &name){QJsonArray a;for(auto &s:name)a.append(s);return a;}
QJsonArray value(const Value &v){
 QJsonValue data;
 switch(v.type().tag){
 case TypeTag::Invalid: data=QJsonValue(); break;
 case TypeTag::Int: case TypeTag::Pixels:data=v.Int();break;
 case TypeTag::Double:data=v.Double();break;
 case TypeTag::Bool:data=v.Bool();break;
 case TypeTag::String:case TypeTag::Align:data=QString::fromStdString(v.String());break;
 case TypeTag::Point:{auto p=v.Point();data=QJsonArray{p.x,p.y};}break;
 case TypeTag::Size:{auto p=v.Size();data=QJsonArray{p.width,p.height};}break;
 case TypeTag::Margins:{auto p=v.Margins();data=QJsonArray{p.left,p.top,p.right,p.bottom};}break;
 case TypeTag::Color:{auto p=v.Color();data=QJsonArray{p.red,p.green,p.blue,p.alpha,p.fallback};}break;
 case TypeTag::Font:{auto p=v.Font();data=QJsonArray{QString::fromStdString(p.family),p.size,p.flags};}break;
 case TypeTag::Struct:{QJsonArray a;if(auto f=v.Fields()){for(auto &field:*f)a.append(QJsonArray{seq(field.variable.name),value(field.variable.value),int(field.status)});}data=a;}break;
 case TypeTag::Icon:{QJsonArray a;for(auto &part:v.Icon().parts){auto filename=part.filename;if(!filename.startsWith("size://"))filename=QDir::cleanPath(filename);a.append(QJsonArray{filename,value(part.color),value(part.padding)});}data=a;}break;
 }
 return {int(v.type().tag),seq(v.type().name),seq(v.copyOf()),data};
}
int main(int argc,char **argv){QCoreApplication app(argc,argv);auto options=parseOptions();QJsonArray modules;
 std::map<QString,std::shared_ptr<const Module>> cache;
 for(int i=0;i<options.inputPaths.size();++i){ParsedFile file(cache,options,i);if(!file.read())return 1;auto m=file.getResult();QJsonArray variables,structs;
 m->enumVariables([&](const Variable &v){variables.append(QJsonArray{seq(v.name),value(v.value),v.description});return true;});
 m->enumStructs([&](const Struct &s){QJsonArray fields;for(auto &f:s.fields)fields.append(QJsonArray{seq(f.name),int(f.type.tag),seq(f.type.name)});structs.append(QJsonArray{seq(s.name),fields});return true;});
 modules.append(QJsonArray{QDir::cleanPath(m->filepath()),structs,variables});}
 QFile out(options.outputPath+"/style-ir.json");if(!out.open(QIODevice::WriteOnly))return 2;out.write(QJsonDocument(modules).toJson(QJsonDocument::Compact));return 0;
}
