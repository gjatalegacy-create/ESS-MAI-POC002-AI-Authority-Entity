"""Validate the exact local native workspace before any publication."""
from pathlib import Path
import hashlib, json, os, re, subprocess, time

project=Path(__file__).resolve().parents[2]
capsule=project/'poc/poc-002-ai-authority-entity'
out=Path(__file__).resolve().parent/'cargo'
out.mkdir(parents=True,exist_ok=True)
tool=Path('C:/Users/AG/.rustup/toolchains/stable-x86_64-pc-windows-gnu/bin')
env={k:v for k,v in os.environ.items() if not k.startswith('ESSMAI_')}
env.update({'PATH':'C:/msys64/mingw64/bin'+os.pathsep+str(tool)+os.pathsep+env['PATH'],
 'RUSTC':str(tool/'rustc.exe'),'RUSTDOC':str(tool/'rustdoc.exe'),
 'CC':'C:/msys64/mingw64/bin/gcc.exe','AR':'C:/msys64/mingw64/bin/ar.exe',
 'CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER':'C:/msys64/mingw64/bin/gcc.exe',
 'CARGO_TARGET_DIR':str(project/'_audit_build_cache/poc002_native_hierarchy'),'CARGO_BUILD_JOBS':'2'})
temp=out/'isolated-temp';temp.mkdir(exist_ok=True)
for key in ('TEMP','TMP','TMPDIR'):env[key]=str(temp)
cmd=[str(tool/'cargo.exe'),'test','--offline','--locked','--workspace','--','--test-threads=1']
start=time.monotonic()
with (out/'workspace-test.log').open('wb') as log:
 p=subprocess.run(cmd,cwd=capsule/'native/runtime',env=env,stdout=log,stderr=subprocess.STDOUT)
raw=(out/'workspace-test.log').read_bytes()
matches=re.findall(rb'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;',raw)
report={'command':cmd,'cwd':str(capsule/'native/runtime'),'exit_code':p.returncode,
 'seconds':round(time.monotonic()-start,2),'summaries':[{'status':a.decode(),'passed':int(b),'failed':int(c),'ignored':int(d)} for a,b,c,d in matches],
 'log_sha256':hashlib.sha256(raw).hexdigest(),'source_changes':0}
(out/'EXECUTION.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
print(json.dumps(report),flush=True)
if p.returncode:print(raw.decode(errors='replace')[-6500:],flush=True)
raise SystemExit(p.returncode)
