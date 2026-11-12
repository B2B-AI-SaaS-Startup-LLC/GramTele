from pathlib import Path
import json,subprocess,sys
fixtures=Path(sys.argv[1]);native=Path(sys.argv[2]);work=Path(sys.argv[3]);work.mkdir(parents=True,exist_ok=True)
cases=[('malformed',b'{broken',False),('empty',b'',False),('bom',b'\xef\xbb\xbf{}',False),('trailing',b'{} ignored trailing',False),('array',b'[]',False),('null',b'null',False),('legacy',b'{"sendReadMessages":false,"sendWithoutSound":true}',False),('legacy-bad-type',b'{"sendReadMessages":1}',False),('ghost-cli',b'{}',True),('ranges',b'{"ghostModeSettings":{},"messageBubbleRadius":99,"wideMultiplier":0.2,"avatarCorners":-1,"translationProvider":"native","messageShotSettings":{"embeddedThemeType":9,"embeddedThemeAccentColor":42,"cloudThemeId":4}}',False),('partial',b'{"ghostModeSettings":{},"saveDeletedMessages":false,"saveMessagesHistory":"bad"}',False),('unicode',(fixtures/'settings-changed.json').read_bytes(),False)]
records=[]
for name,data,ghost in cases:
 path=work/(name+'.json');path.write_bytes(data)
 p=subprocess.run([str(native),'load-ghost' if ghost else 'load',str(path)],capture_output=True,text=True,check=True)
 records.append({'name':name,'input':list(data),'ghost':ghost,'output':json.loads(p.stdout),'written':list(path.read_bytes())})
(fixtures/'load-probes.original.json').write_text(json.dumps(records,ensure_ascii=False,separators=(',',':')))
