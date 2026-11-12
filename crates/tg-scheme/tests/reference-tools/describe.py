#!/usr/bin/env python3
"""Independent, deliberately small raw-TL reader. Does not import tg-tl-gen.
The resulting checked-in JSON is the shared value recipe for C++ and Rust.
"""
import re,json,sys
from pathlib import Path
root=Path(sys.argv[1]);out=Path(sys.argv[2]);defs=[]
for source in ['api.tl','mtproto.tl']:
 function=False
 for line in (root/source).read_text().splitlines():
  line=line.split('//')[0].strip()
  if line=='---functions---':function=True;continue
  if line=='---types---':function=False;continue
  m=re.fullmatch(r'([\w.]+)#([\da-fA-F]+)\s*(.*?)\s*=\s*([^;]+);',line)
  if not m:
   if "#" in line and not line.startswith("vector "):raise ValueError(f"Unrecognized definition: {source}: {line}")
   continue
  name,id,params,result=m.groups()
  if name=='vector':continue
  params=re.sub(r'\{[^}]*\}','',params)
  parsed=[]
  for param in params.split():
   field,ty=param.split(':',1)
   flags=None
   condition=re.fullmatch(r'(\w+)\.(\d+)\?(.+)',ty)
   if condition:flag,bit,ty=condition.groups();flags=[flag,int(bit)]
   parsed.append(dict(name=field,type=ty,condition=flags))
  defs.append(dict(name=name,id=int(id,16),function=function,params=parsed,result=result))
recipe=dict(version=1,source_order=['api.tl','mtproto.tl'],cases=[dict(present=p,length=n,vector=v)for p,n,v in [(False,0,0),(True,253,0),(True,254,2),(True,0,2),(True,3,1),(False,1,1)]],definitions=defs)
out.write_text(json.dumps(recipe,separators=(',',':'))+'\n');print('definitions',len(defs),'constructors',sum(not d['function']for d in defs),'methods',sum(d['function']for d in defs))
