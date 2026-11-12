# Select unchanged serializer functions from the read-only source. UI is not linked.
from pathlib import Path
import sys
root=Path(sys.argv[1]);out=Path(sys.argv[2]);out.mkdir(parents=True,exist_ok=True)
h=(root/'SourceFiles/ayu/ayu_settings.h').read_text();(out/'settings.hpp').write_text(h.replace('private:', 'public:'))
s=(root/'SourceFiles/ayu/ayu_settings.cpp').read_text()
def function(label):
 start=s.index(label);b=s.index('{',start);level=1;i=b+1
 while level:
  if s[i]=='{':level+=1
  elif s[i]=='}':level-=1
  i+=1
 return s[start:i]
labels=['void AyuSettings::load()', 'void AyuSettings::save()', 'void AyuSettings::validate()', 'GhostModeAccountSettings &AyuSettings::ghost(uint64 userId)', 'GhostModeAccountSettings::GhostModeAccountSettings()', 'AyuSettings::AyuSettings()', 'AyuSettings &AyuSettings::getInstance()']
for c in ['GhostModeAccountSettings','MessageShotSettings','AyuSettings']:
 labels += ['void to_json(nlohmann::json &j, const '+c+' &s)', 'void from_json(const nlohmann::json &j, '+c+' &s)']
(out/'settings_functions.cpp').write_text('\n\n'.join(function(x) for x in labels))
