"""Verify the extracted native bytes; optional read-only comparison to source."""
import argparse
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('--source', type=Path)
args = parser.parse_args()
root = Path(__file__).resolve().parent
manifest = json.loads((root / 'provenance/SOURCE_MANIFEST.json').read_text(encoding='utf-8'))
errors = []
expected = {entry['path'] for entry in manifest['files']}
expected.update(manifest.get('packaging_files', {}).keys())
actual = {path.relative_to(root / 'native').as_posix()
          for path in (root / 'native').rglob('*') if path.is_file()
          and 'target' not in path.relative_to(root / 'native').parts}
for path in sorted(actual - expected):
    errors.append(f'extracted: unlisted {path}')
for name, expected_sha in manifest.get('packaging_files', {}).items():
    path = root / 'native' / name
    if not path.is_file() or hashlib.sha256(path.read_bytes()).hexdigest() != expected_sha:
        errors.append(f'packaging: changed {name}')
for entry in manifest['files']:
    for label, base in [('extracted', root / 'native')] + ([('source', args.source)] if args.source else []):
        path = base / entry['path']
        if not path.is_file():
            errors.append(f'{label}: missing {entry["path"]}')
            continue
        raw = path.read_bytes()
        if len(raw) != entry['bytes'] or hashlib.sha256(raw).hexdigest() != entry['sha256']:
            errors.append(f'{label}: changed {entry["path"]}')
if args.source:
    for relative, digest in [('runtime/Cargo.toml', manifest['original_workspace_sha256']),
                             ('runtime/Cargo.lock', manifest['original_lock']['sha256'])]:
        if hashlib.sha256((args.source / relative).read_bytes()).hexdigest() != digest:
            errors.append(f'source: changed {relative}')
print(json.dumps({'files': len(manifest['files']), 'source_checked': args.source is not None,
                  'errors': errors}, indent=2))
raise SystemExit(bool(errors))
