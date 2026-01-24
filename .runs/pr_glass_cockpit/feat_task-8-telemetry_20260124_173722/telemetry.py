import json, sys
from pathlib import Path
numstat = Path(sys.argv[1]).read_text(encoding='utf-8', errors='replace')
files = []
for line in numstat.splitlines():
    p = line.split('\t')
    if len(p) >= 3 and p[0] != '-':
        files.append({'file': p[2], 'delta': int(p[0]) + int(p[1])})
files.sort(key=lambda x: x['delta'], reverse=True)
Path(sys.argv[2]).write_text(json.dumps({'files': len(files)}, indent=2))
lines = ['### Start review here'] + [f'- {x["file"]}' for x in files[:8]]
Path(sys.argv[3]).write_text('\n'.join(lines))
