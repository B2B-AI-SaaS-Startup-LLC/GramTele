#include "settings.hpp"
#include "rpl/combine.h"
#include "rpl/rpl.h"
#include <fstream>
#include <iostream>
namespace AyuAssets { const auto DEFAULT_ICON=QString("default"); }
namespace Core { bool IsAppLaunched(){return false;} }
namespace tr { constexpr auto now=0;QString lng_edited(int){return "edited";} }
namespace { rpl::lifetime lifetime; }
namespace {std::string settingsPath;bool commandGhost=false;std::string getSettingsPath(){return settingsPath;} }
bool cGhost(){return commandGhost;}
namespace Platform {bool IsTranslateProviderAvailable(){return false;}}
namespace AyuUiSettings {constexpr int kMaxAvatarCorners=23;}
GhostModeAccountSettings &AyuSettings::ghost(){return ghost(uint64(0));}
using json=nlohmann::json;
#include "settings_functions.cpp"
int main(int argc,char**argv){
 if(argc!=3)return 1;
 if(std::string(argv[1])=="load" || std::string(argv[1])=="load-ghost"){settingsPath=argv[2];commandGhost=std::string(argv[1])=="load-ghost";AyuSettings::load();std::cout<<nlohmann::json(AyuSettings::getInstance()).dump();return 0;}
 if(std::string(argv[1])=="primitives"){
 std::ofstream out(argv[2]);nlohmann::json records=nlohmann::json::array();
 uint64_t state=123456789;
 for(int i=0;i<2000;++i){state^=state<<13;state^=state>>7;state^=state<<17;double v;std::memcpy(&v,&state,8);if(std::isfinite(v))records.push_back({{"bits",state},{"text",nlohmann::json(v).dump()}});}
 for(uint64_t bits:{0ULL,1ULL,2ULL,0x0010000000000000ULL,0x7fefffffffffffffULL,0x8000000000000000ULL,0x3fb999999999999aULL}){double v;std::memcpy(&v,&bits,8);records.push_back({{"bits",bits},{"text",nlohmann::json(v).dump()}});}out<<records.dump(4);return 0;}
 if(std::string(argv[1])=="probes") {std::ifstream in(argv[2]);nlohmann::json inputs;in>>inputs;auto output=nlohmann::json::array();for(const auto &test:inputs){AyuSettings s;bool error=false;try{from_json(test["input"],s);}catch(...){error=true;}output.push_back({{"name",test["name"]},{"input",test["input"]},{"error",error},{"output",nlohmann::json(s)}});}std::cout<<output.dump();return 0;}
 AyuSettings s;
 if(std::string(argv[1])=="default"){}
 else if(std::string(argv[1])=="changed"){
 nlohmann::json j=s;for(auto&[k,v]:j.items()){
  if(v.is_boolean())v=!v.get<bool>();else if(v.is_number_float())v=2.5;else if(v.is_number())v=(v==0?1:0);else if(v.is_string())v="Привет 🌏\n\t\"\\";
 }
 j["translationProvider"]="yandex";j["shadowBanIds"]={(-9223372036854775807LL-1),0,9223372036854775807LL};
 GhostModeAccountSettings g;nlohmann::json gj=g;for(auto&[k,v]:gj.items()){if(v.is_boolean())v=!v.get<bool>();else v=2;}
 j["ghostModeSettings"]={{"0",gj},{"18446744073709551615",gj}};
 MessageShotSettings m;nlohmann::json mj=m;for(auto&[k,v]:mj.items()){if(v.is_boolean())v=!v.get<bool>();else if(v.is_number())v=18446744073709551615ULL;else v="Тема 🌏";}
 mj["embeddedThemeType"]=2;mj["embeddedThemeAccentColor"]=4294967295U;j["messageShotSettings"]=mj;
 std::ofstream(std::string(argv[2])+".input")<<j.dump(4);from_json(j,s);
 }else if(std::string(argv[1])=="read") {std::ifstream in(argv[2]);nlohmann::json j;in>>j;from_json(j,s);std::cout<<nlohmann::json(s).dump(4);return 0;}else return 2;
 std::ofstream out(argv[2]);out<<nlohmann::json(s).dump(4);
}
