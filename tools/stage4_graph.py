#!/usr/bin/env python3
"""Reproducible lexical include graph; semantic UI classifications remain reviewable evidence."""
import argparse,csv,json,re,collections
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--original',type=Path,required=True);p.add_argument('--output',type=Path,default=Path('docs/stage4'));a=p.parse_args();root=a.original/'Telegram';out=a.output;out.mkdir(parents=True,exist_ok=True)
ledger=list(csv.DictReader(Path('ledger/files.csv').open()));selected={r['path']:r for r in ledger if r['stage']=='4'}
allfiles={str(f.relative_to(root)):f for f in root.rglob('*') if f.is_file() and f.suffix in {'.h','.hpp','.cpp','.cxx'}}
roots=[root/'SourceFiles',*sorted(x for x in root.iterdir()if x.is_dir()and x.name.startswith('lib_')),root]
def resolve(source,inc):
 for base in [allfiles[source].parent,*roots]:
  candidate=base/inc
  if candidate.is_file():return str(candidate.resolve().relative_to(root.resolve()))
 return '@'+inc
edges={};texts={}
for name,path in allfiles.items():
 text=path.read_text(errors='replace');texts[name]=text;edges[name]=sorted(set(resolve(name,i)for i in re.findall(r'^\s*#\s*include\s*["<]([^">]+)[">]',text,re.M)))
ui_prefix=('lib_ui/','SourceFiles/ui/','SourceFiles/window/','SourceFiles/boxes/','SourceFiles/history/view/','SourceFiles/media/view/','SourceFiles/chat_helpers/','SourceFiles/layout/','SourceFiles/dialogs/')
def ui(n):return n.startswith(ui_prefix)
type_headers=('ui/text/text_entity.h','ui/text/text_utilities.h','ui/style/style_core.h','ui/style/style_font.h','ui/style/style_palette.h','ui/effects/animations.h')
def gui(n):return n.startswith('@') and n.rsplit('/',1)[-1] in {'QImage','QPixmap','QColor','QFont','QPainter','QWidget','QApplication','QScreen','QWindow','QIcon','QClipboard','QGuiApplication'}
def type_edge(n):return n.startswith('@Q')and any(t in n for t in ['Image','Pixmap','Color','Font','Size','Rect','Point','Margins']) or any(n.endswith(t)for t in type_headers)
rows=[]
for name,r in selected.items():
 seen=set();todo=list(edges.get(name,[]));direct=[n for n in todo if ui(n)or gui(n)]
 while todo:
  n=todo.pop()
  if n in seen:continue
  seen.add(n);todo.extend(edges.get(n,[]))
 reachable=sorted(n for n in seen if ui(n));substantive=[n for n in direct if (ui(n)or gui(n))and not type_edge(n)]
 category='no-ui' if not reachable and not any(gui(n)for n in seen) else 'types-candidate' if direct and not substantive and all(type_edge(n)for n in direct) else 'substantive-candidate'
 rows.append({'path':name,'lines':int(r['lines']),'category':category,'direct_ui':direct,'reachable_ui':reachable})
# Tarjan SCCs within stage 4.
index={};low={};stack=[];on=set();components=[]
def visit(v):
 index[v]=low[v]=len(index);stack.append(v);on.add(v)
 for w in edges.get(v,[]):
  if w not in selected:continue
  if w not in index:visit(w);low[v]=min(low[v],low[w])
  elif w in on:low[v]=min(low[v],index[w])
 if low[v]==index[v]:
  part=[]
  while True:
   w=stack.pop();on.remove(w);part.append(w)
   if w==v:break
  components.append(sorted(part))
for name in selected:
 if name not in index:visit(name)
types=['TextWithEntities','TextWithTags','QImage','QPixmap','QColor','QFont','style::','Ui::Text::','Ui::RpWidget','Window::SessionController','HistoryView::','ClickHandler','QPainter']
usage={t:{n:len(re.findall(re.escape(t),texts.get(n,'')))for n in selected if t in texts.get(n,'')}for t in types}
# Provisional 4–6k LOC batches, keeping h/cpp pairs together, low dependency groups first.
priority=['lang/','countries/','data/data_peer_id','data/data_msg_id','data/data_changes','data/data_user','data/data_chat','data/data_channel','data/data_photo','data/data_document','data/','history/','api/','apiwrap','main/','core/','ayu/']
def rank(n):
 s=n.removeprefix('SourceFiles/');return next((i for i,x in enumerate(priority)if s.startswith(x)),len(priority)),s
pairs=collections.defaultdict(list)
for n in selected:pairs[str(Path(n).with_suffix(''))].append(n)
groups=sorted(pairs.values(),key=lambda g:rank(g[0]));batches=[];current=[];size=0
for g in groups:
 count=sum(int(selected[n]['lines'])for n in g)
 if current and size+count>6000:batches.append({'files':current,'lines':size});current=[];size=0
 current+=sorted(g);size+=count
if current:batches.append({'files':current,'lines':size})
expanded=[]
for b in batches:
 if b['lines']<=6000:expanded.append(b);continue
 n=(b['lines']+4999)//5000
 for i in range(n):expanded.append({'files':b['files'],'lines':b['lines']//n+(i<b['lines']%n),'part':i+1,'parts':n,'note':'function boundaries require body audit; not literal line cuts'})
batches=expanded
folders=lambda n: n.removeprefix('SourceFiles/').split('/')[0] if '/' in n.removeprefix('SourceFiles/') else '(root)'
folder_edges=sorted(set((folders(n),folders(t))for n in selected for t in edges.get(n,[])if t in selected and folders(n)!=folders(t)))

report={'files':len(rows),'lines':sum(r['lines']for r in rows),'categories':dict(collections.Counter(r['category']for r in rows)),'internal_edges':sum(t in selected for n in selected for t in edges.get(n,[])),'external_edges':sum(t not in selected for n in selected for t in edges.get(n,[])),'sccs':[c for c in components if len(c)>1],'rows':rows,'type_usage':usage,'batches':batches,'folder_edges':folder_edges}
(out/'graph.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');(out/'includes.tsv').write_text('source\ttarget\n'+''.join(n+'\t'+t+'\n'for n in sorted(selected)for t in edges.get(n,[])))
(out/'graph.dot').write_text('digraph stage4 {\n'+''.join(json.dumps(n)+' -> '+json.dumps(t)+';\n'for n in sorted(selected)for t in edges.get(n,[]))+'}\n')
print(json.dumps({k:report[k]for k in ['files','lines','categories','internal_edges','external_edges']},ensure_ascii=False));print('SCC',len(report['sccs']),'batches',len(batches))
