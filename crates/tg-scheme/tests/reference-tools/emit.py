#!/usr/bin/env python3
"""Generate both typed builders from the independent JSON recipe (not tg-tl-gen)."""
import json,re,sys
from pathlib import Path
recipe=json.loads(Path(sys.argv[1]).read_text());dst=Path(sys.argv[2]);dst.mkdir(parents=True,exist_ok=True);defs=recipe['definitions'];byname={d['name']:i for i,d in enumerate(defs)};types={}
for i,d in enumerate(defs):
 if not d['function']:types.setdefault(d['result'],[]).append(i)
renames={'passwordKdfAlgoSHA256SHA256PBKDF2HMACSHA512iter100000SHA256ModPow':'passwordKdfAlgoModPow'}
def name(d):return renames.get(d['name'],d['name']).replace('.','_')
def pascal(s):return ''.join(p[:1].upper()+p[1:]for p in s.replace('.','_').split('_'))
def snake(s):
 s=s.replace('.','_');s=re.sub('([A-Z]+)([A-Z][a-z])',r'\1_\2',s);s=re.sub('([a-z0-9])([A-Z])',r'\1_\2',s);return s.lower()
def cppbox(s):return 'MTP'+'_'.join([*s.split('.')[:-1],s.split('.')[-1][:1].upper()+s.split('.')[-1][1:]])
def typ(t,lang):
 if t.startswith('!'):return 'MTPhelp_GetConfig'if lang=='cpp'else'HelpGetConfig'
 m=re.fullmatch(r'([Vv]ector)<(.+)>',t)
 if m:return ('MTP'+m[1]+'<'+typ(m[2],lang)+'>')if lang=='cpp'else('Vec<'+typ(m[2],lang)+'>'if m[1]=='Vector'else'tl::BareVec<'+typ(m[2],lang)+'>')
 if t in ['int','long','double','string','bytes','int128','int256']:return 'MTP'+t if lang=='cpp'else {'int':'i32','long':'u64','double':'f64','string':'Vec<u8>','bytes':'Vec<u8>','int128':'tl::Int128','int256':'tl::Int256'}[t]
 if t in byname:
  d=defs[byname[t]];return 'MTP'+d['result'][:1].lower()+d['result'][1:] if lang=='cpp'else pascal(name(d))+'Data'
 return cppbox(t)if lang=='cpp'else('bool'if t=='Bool'else pascal(t))
# Fixed-point minimum required-field depth; vectors can always be empty.
cost={}
for _ in range(50):
 for t,indices in types.items():
  for i in indices:
   c=1;ok=True
   for p in defs[i]['params']:
    ty=p['type']
    if p['condition']or ty=='#'or ty.startswith(('Vector<','vector<','!'))or ty in ['int','long','double','string','bytes','int128','int256']:continue
    if ty in types and ty not in cost:ok=False;break
    c+=cost.get(ty,(1,0))[0]
   if ok and(t not in cost or c<cost[t][0]):cost[t]=(c,i)
assert set(types)==set(cost)
def sample(t,lang,tag,nested='true'):
 seed=f'seed("{tag}")';n=f'({seed})'
 if t=='int':return f'MTP_int(int32({n}))'if lang=='cpp'else f'{n} as i32'
 if t=='long':return f'MTP_long(uint64({n})*0x100000001ULL)'if lang=='cpp'else f'({n} as u64)*0x100000001u64'
 if t=='double':return f'MTP_double(double({n}%1000)+0.25)'if lang=='cpp'else f'(({n}%1000) as f64)+0.25'
 if t in ['string','bytes']:return f'MTP_bytes(payload(m,{n}))'if lang=='cpp'else f'payload(m,{n})'
 if t=='int128':return f'MTP_int128({n},uint64({n})*0x100000001ULL)'if lang=='cpp'else f'tl::Int128{{l:{n} as u64,h:({n} as u64)*0x100000001}}'
 if t=='int256':
  a=sample('int128',lang,tag+'a');b=sample('int128',lang,tag+'b');return f'MTP_int256({a},{b})'if lang=='cpp'else f'tl::Int256{{l:{a},h:{b}}}'
 if t.startswith('!'):return 'MTPhelp_GetConfig()'if lang=='cpp'else'HelpGetConfig::new()'
 match=re.fullmatch(r'([Vv]ector)<(.+)>',t)
 if match:
  element=sample(match[2],lang,tag+'[]');count=f'({nested}?0:counts[m])'if lang=='cpp'else f'(if {nested} {{0}} else {{COUNTS[m]}})'
  if lang=='cpp':return f'({count}==0?{typ(t,lang)}():{typ(t,lang)}(MTP_vector<{typ(match[2],lang)}>({count},{element})))'
  data=f'if {count}==0 {{Vec::new()}}else{{vec![{element}; {count}]}}';return data if match[1]=='Vector'else f'tl::BareVec({data})'
 if t in byname:
  i=byname[t]
  return f'{typ(t,lang)}(make_{i}(m,true))'if lang=='cpp'else f'make_{i}(m,true).c_{snake(name(defs[i]))}().clone()'
 return f'make_{cost[t][1]}(m,true)'
def builder(i,lang):
 d=defs[i];s=name(d);outtype=cppbox(d['result'])if not d['function']else cppbox(d['name']);rtype='bool'if d['result']=='Bool'else pascal(d['result'])
 if d['function']:rtype=pascal(s)+('Method'if pascal(s) in {pascal(t)for t in types}else'');outtype=cppbox(d['name']);
 generic=any(p['type'].startswith('!')for p in d['params'])
 if generic:outtype+='<MTPhelp_GetConfig>';rtype+='<HelpGetConfig>'
 bits=0
 for p in d['params']:
  if p['condition']:
   flag,bit=p['condition'];bits|=1<<(bit+(32 if flag=='flags2'else 0))
 args=[]
 for p in d['params']:
  ty=p['type'];cond=p['condition'];tag=d['name']+'.'+p['name']
  if ty=='#':
   if p['name']=='flags2':continue
   if lang=='cpp':
    fclass=('MTP'+s+('<MTPhelp_GetConfig>'if generic else''))if d['function']else'MTPD'+s
    args.append(f'MTP_flags({fclass}::Flags::from_raw(static_cast<{fclass}::Flags::Type>((!nested && present[m])?{bits}ULL:0)))')
   else:args.append(f'tl::Flags::from_raw(if !nested && PRESENT[m] {{{bits}}}else{{0}})'if bits else'tl::Flags::from_raw(0)')
  elif ty=='true':continue
  else:
   val=sample(ty,lang,tag,'nested')
   if cond:
    val=f'((!nested && present[m])?{val}:{typ(ty,lang)}())'if lang=='cpp'else f'if !nested && PRESENT[m] {{Some({val})}}else{{None}}'
   args.append(val)
 if lang=='cpp':
  constructor=('MTP'+s+('<MTPhelp_GetConfig>'if generic else''))if d['function']else'MTP_'+s
  return f'{outtype} make_{i}(unsigned m,bool nested){{return {outtype}({constructor}('+','.join(args)+'));}\n',f'{outtype} make_{i}(unsigned m,bool nested);\n'
 if d['result']=='Bool'and not d['function']:expr='false'if s=='boolFalse'else'true'
 else:expr=(rtype.replace('<','::<')+'::new'if d['function']else'mtp_'+snake(s))+'('+','.join(args)+')'
 return f'fn make_{i}(m:usize,nested:bool)->{rtype}{{{expr}}}\n',''
cpp='''#include "scheme.h"
#include <cstdio>
#include <cstdint>
const bool present[]={false,true,true,true,true,false};
const unsigned lengths[]={0,253,254,0,3,1},counts[]={0,0,2,2,1,1};
uint32_t seed(const char*s){uint32_t h=2166136261U;while(*s){h=(h^uint8_t(*s++))*16777619U;}return h&0x7fffffff;}
QByteArray payload(unsigned m,uint32_t seed){QByteArray b(lengths[m],Qt::Uninitialized);for(int i=0;i<b.size();++i)b[i]=char(seed+17*i);return b;}
'''
rust='''// Generated from the independent JSON recipe; no tg-tl-gen API is used.
#![allow(unused_variables,unused_parens,clippy::clone_on_copy,clippy::double_parens,clippy::unnecessary_cast)]
use tg_scheme::*;use tg_scheme::tl;
const PRESENT:[bool;6]=[false,true,true,true,true,false];
const LENGTHS:[usize;6]=[0,253,254,0,3,1];const COUNTS:[usize;6]=[0,0,2,2,1,1];
fn seed(s:&str)->u32{s.bytes().fold(2166136261u32,|h,c|(h^c as u32).wrapping_mul(16777619))&0x7fffffff}
fn payload(m:usize,seed:u32)->Vec<u8>{(0..LENGTHS[m]).map(|i|seed.wrapping_add(17*i as u32)as u8).collect()}
'''
cpp_funcs=[]
for i in range(len(defs)):
 c,decl=builder(i,'cpp');cpp+=decl;cpp_funcs.append(c);rust+=builder(i,'rust')[0]
cpp+=''.join(cpp_funcs)
cpp+='''template<class T>void capture_value(FILE*f,const T&v){mtpBuffer b;v.write(b);uint32_t n=b.size()*4;std::fwrite(&n,4,1,f);std::fwrite(b.data(),1,n,f);}
int main(int argc,char**argv){if(argc!=2)return 1;FILE*f=std::fopen(argv[1],"wb");if(!f)return 2;'''
rust+='pub fn sample(index:usize,m:usize)->Vec<i32>{match index{\n'
for i in range(len(defs)):
 cpp+=f'for(unsigned m=0;m<6;++m)capture_value(f,make_{i}(m,false));\n'
 rust+=f'{i}=>tl::serialize(&make_{i}(m,false)),\n'
cpp+='std::fclose(f);}\n';rust+='_=>panic!("unknown recipe definition")}}\n'
rust+='\npub fn reread(index:usize,data:&[i32])->tl::Result<Vec<i32>>{Ok(match index{\n'
for i,d in enumerate(defs):
 ty=pascal(name(d))+('Method'if pascal(name(d)) in {pascal(t)for t in types}else'') if d['function'] else('bool'if d['result']=='Bool'else pascal(d['result']))
 if any(p['type'].startswith('!')for p in d['params']):ty+='<HelpGetConfig>'
 rust+=f'{i}=>tl::serialize(&tl::deserialize_exact::<{ty}>(data)?),\n'
rust+='_=>panic!("recipe index")})}\n'
(dst/'builders.cpp').write_text(cpp);(dst/'builders.rs').write_text(rust)
print('emitted',len(defs),'typed builders')
