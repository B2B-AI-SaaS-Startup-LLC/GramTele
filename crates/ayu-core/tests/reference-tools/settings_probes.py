from pathlib import Path
import copy,json,sys
fixtures=Path(sys.argv[1]);base=json.loads((fixtures/'settings-changed.json.input').read_text());cases=[]
def add(name,obj):cases.append({'name':name,'input':obj})
for key in base:
 if key in ['ghostModeSettings','messageShotSettings']:continue
 for suffix,value in [('null',None),('object',{}),('wrong-bool',True)]:
  j=copy.deepcopy(base);j[key]=value;add(key+'/'+suffix,j)
for group,obj in [('ghostModeSettings',base['ghostModeSettings']['0']),('messageShotSettings',base['messageShotSettings'])]:
 for key in obj:
  for suffix,value in [('null',None),('object',{})]:
   j=copy.deepcopy(base)
   if group=='ghostModeSettings':j[group]['0'][key]=value
   else:j[group][key]=value
   add(group+'/'+key+'/'+suffix,j)
for name,obj in [('unknown',{'unknown':1}),('null',None),('array',[]),('legacy-theme-bad',{'messageShotSettings':{'embeddedThemeType':2,'themeType':'bad'}}),('account-id',{'ghostModeSettings':{' -1tail':{}}}),('account-overflow',{'ghostModeSettings':{'18446744073709551616':{}}}),('shadow-collisions',{'shadowBanIds':[1,14,27,0,13,26,-1,-14,-27]}),('shadow-rehash',{'shadowBanIds':list(range(300))})]:add(name,obj)
(fixtures/'settings-probes.input.json').write_text(json.dumps(cases,ensure_ascii=False,separators=(',',':')))
