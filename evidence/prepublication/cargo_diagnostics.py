"""Preserve both the first failure and an unchanged complete-workspace rerun."""
from pathlib import Path
import hashlib,json,os,re,subprocess,time
base=Path(__file__).resolve().parent
project=base.parents[1]
capsule=project/'poc/poc-002-ai-authority-entity'
tool=Path('C:/Users/AG/.rustup/toolchains/stable-x86_64-pc-windows-gnu/bin')
env={k:v for k,v in os.environ.items() if not k.startswith('ESSMAI_')}
env.update({'PATH':'C:/msys64/mingw64/bin'+os.pathsep+str(tool)+os.pathsep+env['PATH'],
 'RUSTC':str(tool/'rustc.exe'),'RUSTDOC':str(tool/'rustdoc.exe'),
 'CC':'C:/msys64/mingw64/bin/gcc.exe','AR':'C:/msys64/mingw64/bin/ar.exe',
 'CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER':'C:/msys64/mingw64/bin/gcc.exe',
 'CARGO_TARGET_DIR':str(project/'_audit_build_cache/poc002_native_hierarchy'),'CARGO_BUILD_JOBS':'2'})
out=base/'cargo-diagnostics';out.mkdir(exist_ok=True)
temp=out/'isolated-temp';temp.mkdir(exist_ok=True)
for key in ('TEMP','TMP','TMPDIR'):env[key]=str(temp)
commands=[('isolated-deadline',['test','--offline','--locked','-p','light-platform','--test','process_deadline_contracts','blocked_stdin_obeys_deadline_and_retains_output','--','--exact','--test-threads=1']),
 ('workspace-complete',['test','--offline','--locked','--workspace','--no-fail-fast','--','--test-threads=1'])]
reports=[]
for name,args in commands:
 cmd=[str(tool/'cargo.exe'),*args];start=time.monotonic();log=out/(name+'.log')
 with log.open('wb') as stream:
  p=subprocess.run(cmd,cwd=capsule/'native/runtime',env=env,stdout=stream,stderr=subprocess.STDOUT)
 raw=log.read_bytes();matches=re.findall(rb'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;',raw)
 item={'command':cmd,'exit_code':p.returncode,'seconds':round(time.monotonic()-start,2),
  'summaries':[{'status':a.decode(),'passed':int(b),'failed':int(c),'ignored':int(d)} for a,b,c,d in matches],
  'log_sha256':hashlib.sha256(raw).hexdigest(),'runtime_source_changes':0}
 (out/(name+'.json')).write_text(json.dumps(item,indent=2)+'\n',encoding='utf-8');reports.append(item)
 print(json.dumps(item),flush=True)
 if p.returncode:print(raw.decode(errors='replace')[-4500:],flush=True)
(out/'ALL.json').write_text(json.dumps(reports,indent=2)+'\n',encoding='utf-8')
