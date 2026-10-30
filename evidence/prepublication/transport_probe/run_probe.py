"""Bounded diagnostics using unchanged native transport; all generated files stay here."""
from pathlib import Path
import hashlib
import json
import os
import subprocess
import time
from datetime import datetime, timezone

base = Path(__file__).resolve().parent
project = base.parents[2]
native = project / 'poc/poc-002-ai-authority-entity/native/runtime/light'
transport = native / 'src/process_deadline.rs'
source = native / 'tests/fixtures/process_deadline_child.rs'
tool = Path('C:/Users/AG/.rustup/toolchains/stable-x86_64-pc-windows-gnu/bin')
rustc = tool / 'rustc.exe'
env = dict(os.environ)
env['PATH'] = 'C:/msys64/mingw64/bin' + os.pathsep + str(tool) + os.pathsep + env['PATH']
for key in ('TEMP', 'TMP', 'TMPDIR'):
    env[key] = str(base)

def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()

before = {str(p): sha(p) for p in (transport, source)}
began = time.monotonic()
commands = []

def run(name, args, timeout=40):
    if time.monotonic() - began > 240:
        raise RuntimeError('five-minute diagnostic budget nearly exhausted')
    started = datetime.now(timezone.utc).isoformat()
    t = time.monotonic()
    result = subprocess.run([str(a) for a in args], cwd=base, env=env, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, timeout=timeout)
    (base / (name + '.stdout.log')).write_bytes(result.stdout)
    (base / (name + '.stderr.log')).write_bytes(result.stderr)
    entry = {'name': name, 'started_at_utc': started, 'seconds': round(time.monotonic()-t, 6),
             'command': [str(a) for a in args], 'exit_code': result.returncode,
             'stdout_file': name + '.stdout.log', 'stderr_file': name + '.stderr.log'}
    commands.append(entry)
    print(json.dumps(entry), flush=True)
    if result.returncode:
        raise RuntimeError(result.stderr.decode(errors='replace'))
    return result

run('toolchain', [rustc, '--version'])
run('build-probe', [rustc, '--edition=2021', base/'probe.rs', '-o', base/'probe.exe'])
original = source.read_bytes()
(base/'original_child.rs').write_bytes(original)
assert sha(base/'original_child.rs') == before[str(source)]
run('build-original-a', [rustc, '--edition=2021', base/'original_child.rs', '-o', base/'original_a.exe'])
run('original-a-first', [base/'probe.exe', base/'original_a.exe', 'original-a-first'])
run('original-a-second', [base/'probe.exe', base/'original_a.exe', 'original-a-second'])
run('build-original-b', [rustc, '--edition=2021', base/'original_child.rs', '-o', base/'original_b.exe'])
run('original-b-first', [base/'probe.exe', base/'original_b.exe', 'original-b-first'])

# Only this external fixture is instrumented. Native fixture and transport remain untouched.
diagnostic = original.decode('utf-8')
marker = '''
fn diagnostic_mark(label: &str) {
    if let Ok(path) = std::env::var("DIAGNOSTIC_MARKERS") {
        let ns = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "{} {} {}", label, ns, std::process::id());
            let _ = f.flush();
        }
    }
}
'''
diagnostic = diagnostic.replace('fn main() {', marker + '\nfn main() {\n    diagnostic_mark("MAIN_ENTERED");', 1)
diagnostic = diagnostic.replace('std::io::stdout().flush().unwrap();', 'std::io::stdout().flush().unwrap(); diagnostic_mark("STDOUT_FLUSHED");', 1)
diagnostic = diagnostic.replace('eprintln!("stderr-before-wait");', 'eprintln!("stderr-before-wait"); diagnostic_mark("STDERR_WRITTEN");', 1)
(base/'diagnostic_child.rs').write_text(diagnostic, encoding='utf-8')
run('build-diagnostic', [rustc, '--edition=2021', base/'diagnostic_child.rs', '-o', base/'diagnostic.exe'])
run('diagnostic-first', [base/'probe.exe', base/'diagnostic.exe', 'diagnostic-first', base/'diagnostic-first.markers'])
run('diagnostic-second', [base/'probe.exe', base/'diagnostic.exe', 'diagnostic-second', base/'diagnostic-second.markers'])
after = {str(p): sha(p) for p in (transport, source)}
assert before == after
report = {'recorded_at_utc': datetime.now(timezone.utc).isoformat(),
          'method': 'Exact native transport included by path; exact original fixture for first three runs; only external diagnostic fixture adds timestamps.',
          'time_budget_ms_unchanged': 750, 'test_assertions_or_runtime_changed': False,
          'native_before': before, 'native_after': after, 'commands': commands,
          'total_seconds': round(time.monotonic()-began, 3)}
(base/'DIAGNOSTIC_RECEIPT.json').write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')
print(json.dumps({'status': 'diagnostics_finished', 'native_unchanged': before == after,
                  'seconds': report['total_seconds']}), flush=True)
