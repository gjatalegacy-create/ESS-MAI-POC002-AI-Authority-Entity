"""Five explicit user turns through unchanged native binaries and isolated Vault.

This harness supplies inputs and inspects files; it never grants authority,
seeds candidates, changes hardware limits, or decides a Shadow result.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

def sha(raw):
    return hashlib.sha256(raw).hexdigest()

def save(path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--build', type=Path, required=True)
    ap.add_argument('--output', type=Path, required=True)
    args = ap.parse_args()
    build, out = args.build.resolve(), args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    for name in ('tmp', 'handoff'):
        (out/name).mkdir()
    env = {k:v for k,v in os.environ.items() if not k.startswith('ESSMAI_')}
    settings = {'ESSMAI_VAULT':str(out/'shadow.svlt'), 'ESSMAI_HANDOFF_DIR':str(out/'handoff'),
        'ESSMAI_QUANTUM_BIN':str(build/'quantum-platform.exe'),
        'ESSMAI_SHADOW_BIN':str(build/'shadow_platform.exe'),
        'TEMP':str(out/'tmp'), 'TMP':str(out/'tmp'), 'TMPDIR':str(out/'tmp')}
    env.update(settings)
    env['PATH'] = 'C:/msys64/mingw64/bin' + os.pathsep + env.get('PATH','')
    save(out/'environment.json', settings)
    names = ['light-platform.exe','quantum-platform.exe','shadow_platform.exe',
             'examples/pd_iz_input.exe','examples/pd_recovery_inspect.exe']
    save(out/'binaries.json', {n:sha((build/n).read_bytes()) for n in names})
    light, shadow = build/names[0], build/names[2]
    inspect, recovery = build/names[3], build/names[4]
    calls, turns, checks = [], [], []
    def call(label, executable, *argv, expect=None):
        cmd = list(map(str,(executable,*argv)))
        start = time.monotonic()
        with (out/(label+'.stdout')).open('wb') as stdout, (out/(label+'.stderr')).open('wb') as stderr:
            p = subprocess.run(cmd, cwd=out, env=env, stdin=subprocess.DEVNULL,
                stdout=stdout, stderr=stderr, timeout=240)
        row = {'step':label,'command':cmd,'exit_code':p.returncode,'seconds':round(time.monotonic()-start,3)}
        calls.append(row); save(out/'commands.json',calls)
        print(json.dumps(row),flush=True)
        if expect is None and p.returncode != 0:
            raise RuntimeError(label + ': ' + (out/(label+'.stderr')).read_text(encoding='utf-8',errors='replace')[-3500:])
        if expect == 'refusal' and p.returncode == 0:
            raise RuntimeError(label + ': expected refusal, got success')
        if expect == 'refusal':
            reason=(out/(label+'.stderr')).read_text(encoding='utf-8',errors='strict')
            if p.returncode != 70 or 'PD_T1_REQUEST_CONFLICT' not in reason:
                raise RuntimeError(label + ': refusal was not the required request-binding conflict: '+reason)
        return (out/(label+'.stdout')).read_text(encoding='utf-8',errors='strict')
    def measure(label):
        files = {}
        for p in out.glob('shadow.svlt*'):
            if p.is_file():
                files[p.name]={'bytes':p.stat().st_size,'sha256':sha(p.read_bytes())}
        save(out/(label+'.memory.json'),files)
        return files
    def operational(n, text, parent=None, native=False):
        raw, output, reference = out/f'c{n}.txt', out/f'c{n}.lgc', out/f'c{n}.ref'
        raw.write_text(text,encoding='utf-8')
        mode = '--iz-native-reply-once' if native else '--iz-lgc-reply-once' if parent else '--iz-lgc-new-once'
        argv = [mode, f'poc002-c{n}'] + ([parent] if parent else []) + [raw,output]
        call(f'c{n}-run',light,*argv)
        data=json.loads(call(f'c{n}-inspect',inspect,'inspect-lgc-result',output))
        save(out/f'c{n}.inspection.json',data)
        if data['status'] != 'OperationalNoVerdict':
            raise RuntimeError(f'c{n}: native path changed; inspect result before deriving next reference')
        call(f'c{n}-ref',inspect,'export-lgc-result',output,reference)
        turns.append({'turn':n,'input':text,'input_sha256':sha(raw.read_bytes()),'kind':'operational','inspection':f'c{n}.inspection.json'})
        save(out/'turns.json',turns); measure(f'c{n}')
        return reference
    def scientific(n, parent, claim):
        raw, record, lookup, bundle = out/f'c{n}.json',out/f'c{n}.pds',out/f'c{n}.lookup',out/f'c{n}.bundle'
        text=f'Evaluate the exact claim {claim} radian, using UT1 and tolerance 1e-12 radian. Model only.'
        save(raw, {'user_text':text,'answer':{'positive':f'exact claim {claim} radian,','verification':'tolerance 1e-12 radian.'},
          'scientific_query':{'task':'earth_rotation_angle','time_scale':'UT1','angle_unit':'radian',
          'jd_part1':2400000.5,'jd_part2':54388.0,'claim_radians':claim,'tolerance_radians':1e-12,
          'input_source_id':'declared numerical experiment, no physical observation','query_id':f'poc002-c{n}',
          'relevance_statement':'Same Earth rotation investigation, restricted to the declared era00 model; user labels are unverified.'}})
        call(f'c{n}-science',light,'--iz-science-answer-once',f'poc002-c{n}',parent,raw,record)
        data=json.loads(call(f'c{n}-science-inspect',inspect,'inspect-science',record))
        save(out/f'c{n}.science.inspection.json',data)
        call(f'c{n}-native',light,'--kie-cycle-once',record,lookup)
        if lookup.read_bytes()[:4] != b'PDRL':
            raise RuntimeError(f'c{n}: operational refusal, not a native lookup; retain output without forcing promotion')
        call(f'c{n}-recover',light,'--pd-native-recover-once',lookup,bundle)
        data=json.loads(call(f'c{n}-inspect',recovery,bundle))
        save(out/f'c{n}.inspection.json',data)
        turns.append({'turn':n,'input':text,'input_sha256':sha(text.encode()),'kind':'native','inspection':f'c{n}.inspection.json',
                      'verdict':[data['y_verdict'],data['x_verdict']]})
        save(out/'turns.json',turns); measure(f'c{n}')
        return lookup,data
    try:
        c1=operational(1,'Evaluate Earth rotation angle under the declared mathematical model only.')
        c2=operational(2,'The Earth rotation angle must be checked under the same declared model. I do not yet supply a numerical claim or a measured observation.',c1)
        c3,d3=scientific(3,c2,0.40228372400281387)
        c4=operational(4,'For the same Earth rotation angle investigation, evaluate a new opposing numerical claim; I have not supplied that claim yet.',c3,True)
        c5,d5=scientific(5,c4,0.0)
        assert d5['predecessor_identity'] == d3['bundle_identity']
        assert d5['cognitive_session_id'] == d3['cognitive_session_id']
        checks.append({'check':'same_case_native_predecessor','pass':True})
        for n,lookup,data in ((3,c3,d3),(5,c5,d5)):
            request=out/'handoff/native_pd_intents'/f"{data['bundle_identity']}-{data['request_sha256']}.pdt1"
            assert request.is_file()
            before=measure(f'c{n}-replay-before')
            response=out/f'c{n}-replayed-response.bin'
            call(f'c{n}-replay-t1',shadow,'--bridge-pd-once',request,response)
            assert sha(response.read_bytes()) == data['shadow_response_sha256']
            # pd_cycle::decode_finalize_lookup requires PDL2; recovery uses PDRL.
            # Both carry the same identity and request SHA in the remaining 64 bytes.
            raw=lookup.read_bytes(); assert len(raw)==68 and raw[:4]==b'PDRL'
            finalize=out/f'c{n}.finalize.lookup'
            finalize.write_bytes(b'PDL2'+raw[4:])
            for retry in (1,2):
                cont=out/f'c{n}-replayed-t2-{retry}.bin'
                call(f'c{n}-replay-t2-{retry}',shadow,'--pd-finalize-once',finalize,cont)
                assert sha(cont.read_bytes()) == data['continuation_sha256']
            after=measure(f'c{n}-replay-after')
            assert before == after and before
            checks.append({'check':f'c{n}_exact_t1_t2_retry','pass':True,'memory_unchanged':True})
            bad=out/f'c{n}-conflicting.lookup'; raw=bytearray(finalize.read_bytes()); assert len(raw)==68
            raw[-1] ^= 1; bad.write_bytes(raw)
            call(f'c{n}-conflict',shadow,'--pd-finalize-once',bad,out/f'c{n}-conflict-output.bin',expect='refusal')
            assert not (out/f'c{n}-conflict-output.bin').exists()
            assert measure(f'c{n}-conflict-after') == after
            checks.append({'check':f'c{n}_same_identity_changed_request_sha','pass':True,'memory_unchanged':True})
        save(out/'RESULT.json',{'complete':True,'user_turns':5,'native_terminal_cycles':2,'checks':checks,
          'scope':'One case, live hardware, native default kernels, fresh isolated Vault. Numerical model input is not physical observation. No forced verdict or native-code edits.'})
    except Exception as error:
        save(out/'RESULT.json',{'complete':False,'completed_user_turns':len(turns),'checks':checks,'error':str(error)})
        raise

if __name__ == '__main__':
    main()
