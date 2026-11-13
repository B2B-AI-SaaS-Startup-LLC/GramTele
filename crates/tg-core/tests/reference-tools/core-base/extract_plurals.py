"""Extract original pure functions, unchanged, for the native reference target."""
import sys
from pathlib import Path
s=(Path(sys.argv[1])/'SourceFiles/lang/lang_tag.cpp').read_text()
body=s[s.index('constexpr auto kShiftZero'):s.index('const auto ChoosePluralDefault')]
main=r'''
int main() {
 const auto rules=GeneratePluralRulesMap();
 for(const auto &[key,rule]:rules) for(int i=0;i<=220;++i) for(int v=0;v<=2;++v){
  const auto n=v?-1:i, f=v?i:0;
  std::cout<<uint64(key)<<" "<<n<<" "<<i<<" "<<v<<" "<<v<<" "<<f<<" "<<f<<" "<<rule(n,i,v,v,f,f)<<"\n";
 }
}
'''
Path(sys.argv[2]).write_text('#include <QtCore/QString>\n#include <map>\n#include <iostream>\nusing uint64=uint64_t;\n'+body+main)
