"""Read-only consistency check of the recorded native five-turn experiment."""
import argparse
import hashlib
import json
from pathlib import Path

def digest(raw):
    return hashlib.sha256(raw).hexdigest()

ap=argparse.ArgumentParser()
ap.add_argument('--evidence',type=Path,default=Path(__file__).resolve().parent/'evidence/continuity')
args=ap.parse_args()
root=args.evidence
def read(name):
    return json.loads((root/name).read_text(encoding='utf-8'))

commands={item['step']:item for item in read('commands.json')}
turns=read('turns.json')
assert [x['turn'] for x in turns]==[1,2,3,4,5]
records={n:read(f'c{n}.inspection.json')['record'] for n in (1,2,4)}
native={n:read(f'c{n}.inspection.json') for n in (3,5)}
identity=records[1]['identity_key']
assert all(x['identity_key']==identity for x in records.values())
assert all(x['cognitive_session_id']==identity for x in native.values())
for n,r in records.items():
    assert r['hold'] is True and r['knowledge_verdict'] is None and r['knowledge_write']==0
    assert r['actual_user_text']==turns[n-1]['input']
    assert digest(r['actual_user_text'].encode())==turns[n-1]['input_sha256']
assert records[2]['previous_record']==records[1]['record_id']
assert records[4]['native_anchor']==native[3]['bundle_identity']
assert native[5]['predecessor_identity']==native[3]['bundle_identity']
assert native[5]['predecessor_request_sha256']==native[3]['request_sha256']
for n in (3,5):
    science=read(f'c{n}.science.inspection.json')
    data=native[n]
    assert science['identity_key']==identity
    assert science['record_id']==data['source_dialogue_record_id']
    assert science['actual_user_text']==turns[n-1]['input']
    assert science['previous_record']==records[n-1]['record_id']
    assert data['continuation_sha256'] and data['failure_witness_lgc']
    assert data['y_verdict']==data['x_verdict'] and data['y_verdict'] in (0,1)
    assert 'backend=C_KERNEL; sealed=PASS; stripped=BLOCK; freeze=BLOCK; recovery=PASS' in (root/f'c{n}-native.stdout').read_text(encoding='utf-8')
    assert digest((root/f'c{n}-replayed-response.bin').read_bytes())==data['shadow_response_sha256']
    for k in (1,2):
        assert digest((root/f'c{n}-replayed-t2-{k}.bin').read_bytes())==data['continuation_sha256']
    before=read(f'c{n}-replay-before.memory.json')
    assert before and before==read(f'c{n}-replay-after.memory.json')==read(f'c{n}-conflict-after.memory.json')
    assert commands[f'c{n}-conflict']['exit_code']==70
    assert 'PD_T1_REQUEST_CONFLICT' in (root/f'c{n}-conflict.stderr').read_text(encoding='utf-8')
    assert not (root/f'c{n}-conflict-output.bin').exists()
    lookup=(root/f'c{n}.lookup').read_bytes()
    finalize=(root/f'c{n}.finalize.lookup').read_bytes()
    bad=(root/f'c{n}-conflicting.lookup').read_bytes()
    assert len(lookup)==len(finalize)==len(bad)==68
    assert lookup[:4]==b'PDRL' and finalize==b'PDL2'+lookup[4:]
    assert bad[:-1]==finalize[:-1] and bad[-1]==finalize[-1]^1
for key,value in commands.items():
    assert value['exit_code']==(70 if key.endswith('-conflict') else 0)
wal=(root/'shadow.svlt').read_bytes()
assert read('c5-replay-after.memory.json')['shadow.svlt']=={'bytes':len(wal),'sha256':digest(wal)}
# A canonical model rendering is an extra native checkpoint entry, not a new user.
trace=native[5]['trace_user_texts']
assert len(trace)==7 and native[5]['session_turn_count']==7
assert [trace[k] for k in (0,1,2,4,5)]==[x['input'] for x in turns]
print(json.dumps({'evidence_consistent':True,'user_turns':5,'operational_holds':3,
  'native_terminal_cycles':2,'native_verdicts':[[native[n]['y_verdict'],native[n]['x_verdict']] for n in (3,5)],
  'replay_response_bytes_identical':True,'conflicting_request_refused_at_expected_guard':True,
  'vault_bytes_unchanged_on_retries_and_conflicts':True,'wal_bytes':len(wal),'wal_sha256':digest(wal),
  'one_cognitive_case':identity,'checkpoint_entries':7},indent=2))
