from pathlib import Path
import re,json,hashlib,shutil,sys
src=Path(sys.argv[1]);dst=Path(sys.argv[2])
dst.mkdir(parents=True,exist_ok=True)
def block(s,name):return re.search(r'\b'+name+r'\[\]\s*=\s*\{(.*?)\};',s,re.S)[1]
def nums(s):return [int(x,0) for x in re.findall(r'0x[0-9a-fA-F]+|\b\d+\b',s)]
# Native C++ arrays: cast names contain digits (uint32), avoid matching type widths.
def values(s):return [int(x,0) for x in re.findall(r'(?<![a-zA-Z0-9_])(0x[0-9a-fA-F]+|\d+)(?:U)?\b',s)]
lang=(src/'lang/lang_auto.cpp').read_text();emoji=(src/'emoji/emoji.cpp').read_text();suggestions=(src/'emoji/emoji_suggestions_data.cpp').read_text();palette=(src/'palette/palette.cpp').read_text();header=(src/'palette/palette.h').read_text()
result={'lang':{n:values(block(lang,n)) for n in ['DefaultData','Offsets']},'emoji':{n:values(block(emoji,n)) for n in ['IdData','Data','SectionData']},'suggestions':{n:values(block(suggestions,n)) for n in ['ReplacementData','ReplacementWordLengths','ReplacementInitData','ReplacementIndices','ReplacementChecksums','ReplacementIndexData']}}
rows=re.findall(r'that.compute\((\d+), (-?\d+), \{\s*(\d+), (\d+), (\d+), (\d+)\s*\}\)',palette)
colors={int(i):name for name,i in re.findall(r'const color &([A-Za-z_0-9]+)\(\) const \{ return _colors\[(\d+)\]',header)}
# Includes special colors whose methods may have the same inline spelling.
result['palette']={'checksum':int(re.search(r'palette_data::Checksum\(\) \{\s*return (-?\d+);',palette)[1]),'colors':[[colors[int(r[0])],int(r[0]),int(r[1]),*[int(v)for v in r[2:]]]for r in rows]}
(dst/'data.json').write_text(json.dumps(result,separators=(',',':'))+'\n')
for name in ['lang_auto.tags','lang_auto.indices']:shutil.copyfile(src/'lang'/name,dst/name)
manifest={str(p.relative_to(src)):hashlib.sha256(p.read_bytes()).hexdigest()for p in sorted(src.rglob('*')) if p.is_file()}
(dst/'original-output-sha256.json').write_text(json.dumps(manifest,indent=2)+'\n')
print('extracted',[(k,len(v))for k,v in result['lang'].items()],len(rows),'palette')

# Normalize only the source-root prefix in the parser IR, preserving all data.
if (src/'styles/style-ir.json').is_file():
 data=json.loads((src/'styles/style-ir.json').read_text())
 prefix=sys.argv[3].rstrip('/')+'/Telegram/'
 def normalize(v):
  if isinstance(v,str):return v.removeprefix(prefix)
  if isinstance(v,list):return [normalize(x)for x in v]
  return v
 (dst/'style-ir.json').write_text(json.dumps(normalize(data),ensure_ascii=False,separators=(',',':'))+'\n')

if (src/'emoji/emoji-ir.json').is_file():
 shutil.copyfile(src/'emoji/emoji-ir.json',dst/'emoji-ir.json')
