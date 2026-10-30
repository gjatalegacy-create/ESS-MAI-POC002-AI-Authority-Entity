from pathlib import Path
import json, os, subprocess, time
from datetime import datetime, timezone
base=Path(__file__).resolve().parent
tool=Path('C:/Users/AG/.rustup/toolchains/stable-x86_64-pc-windows-gnu/bin')
env=dict(os.environ)
env['PATH']='C:/msys64/mingw64/bin'+os.pathsep+str(tool)+os.pathsep+env['PATH']
for k in ('TEMP','TMP','TMPDIR'):env[k]=str(base)
records=[]
def run(name,args):
    begin=time.monotonic()
    p=subprocess.run([str(a) for a in args],cwd=base,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=30)
    (base/(name+'.stdout.log')).write_bytes(p.stdout)
    (base/(name+'.stderr.log')).write_bytes(p.stderr)
    record={'name':name,'command':[str(a) for a in args],'exit_code':p.returncode,'seconds':round(time.monotonic()-begin,3)}
    records.append(record)
    print(json.dumps(record),flush=True)
    assert p.returncode == 0, p.stderr.decode(errors='replace')
run('build-spawn-timing',[tool/'rustc.exe','--edition=2021',base/'spawn_timing.rs','-o',base/'spawn_timing.exe'])
for suffix in ('c','d'):
    binary=base/('diagnostic_'+suffix+'.exe')
    run('build-spawn-'+suffix,[tool/'rustc.exe','--edition=2021',base/'diagnostic_child.rs','-o',binary])
    for attempt in ('first','second'):
        name='spawn-'+suffix+'-'+attempt
        run(name,[base/'spawn_timing.exe',binary,base/(name+'.markers')])
(base/'SPAWN_CONTROL_RECEIPT.json').write_text(json.dumps({'recorded_at_utc':datetime.now(timezone.utc).isoformat(),
    'scope':'Diagnostic control measures Command::spawn separately, then kills at the same 750ms budget or immediately after a late spawn. No native code changes; not a replacement transport test.',
    'commands':records},indent=2)+'\n',encoding='utf-8')
