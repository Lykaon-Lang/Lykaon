import os, glob, json, hashlib, subprocess, tempfile, re

root = r'C:\Users\emran\OneDrive\Documents\LYKAON'
cli = os.path.join(root, 'Lykaon', 'target', 'debug', 'lykaon-cli.exe')
out_dir = os.path.join(root, 'tools', 'corpus')
os.makedirs(out_dir, exist_ok=True)

cases = []
seen = set()

def add(code, expected, source):
    key = hashlib.sha256(code.encode('utf-8')).hexdigest()
    if key in seen:
        return
    low = code.lower()
    skip = ['use ', 'load ', 'ask ', ' file ', 'read ', 'write ', 'append ',
            'run ', 'serve', 'fetch', 'post', 'open database', 'plot ',
            'predict ', 'save model', 'when ', 'start task', 'send ',
            'receive', 'sleep', 'after ', 'every ', 'random', 'environment',
            'exit', 'log ', 'sqlite', 'postgres', 'limit ', 'encrypted ']
    if any(w in low for w in skip):
        return
    seen.add(key)
    cases.append((code, expected, source))

# docs/examples (50)
for f in sorted(glob.glob(os.path.join(root, 'docs', 'examples', '*.lykaon'))):
    out = f[:-6] + '.out'
    if os.path.exists(out):
        add(open(f, encoding='utf-8').read(), open(out, encoding='utf-8').read(), os.path.relpath(f, root))

# conformance valid/run suites
for suite in ['m1b/valid', 'm2/run', 'm2/test', 'm2/valid', 'm3/run']:
    for d in sorted(glob.glob(os.path.join(root, 'Lykaon', 'tests', 'conformance', suite, '*'))):
        inp = os.path.join(d, 'input.lykaon')
        exp = os.path.join(d, 'expected.txt')
        if os.path.exists(inp) and os.path.exists(exp):
            add(open(inp, encoding='utf-8').read(), open(exp, encoding='utf-8').read(),
                os.path.relpath(inp, root))

# examples/m5
for f in sorted(glob.glob(os.path.join(root, 'examples', 'm5', '*.lykaon'))):
    if os.path.basename(f) == 'web-server.lykaon':
        continue
    add(open(f, encoding='utf-8').read(), '', os.path.relpath(f, root))

passed = []
tmp = os.path.join(tempfile.gettempdir(), 'lykaon_corpus_case.lykaon')
for code, expected, source in cases:
    open(tmp, 'w', encoding='utf-8', newline='\n').write(code)
    try:
        r = subprocess.run([cli, 'run', tmp], capture_output=True, text=True, timeout=10)
    except Exception:
        continue
    got = r.stdout.strip()
    if r.returncode == 0 and got == expected.strip():
        passed.append((code, expected.strip(), source))

features = ['say', 'ask', 'set', 'if', 'repeat', 'for', 'to ', 'list', 'map',
            'record', 'enum', 'try', 'test', 'channel', 'task', 'json', 'csv',
            'file', 'text', 'contract', 'generic']
records = []
for i, (code, expected, source) in enumerate(passed):
    low = code.lower()
    feats = [f.strip() for f in features if f in low]
    lines = code.count('\n') + 1
    difficulty = 'easy' if lines < 10 else ('medium' if lines < 30 else 'hard')
    records.append({
        'id': 'c%04d' % i,
        'task': source,
        'code': code,
        'expected': expected,
        'difficulty': difficulty,
        'features': feats,
        'license': 'CC0-1.0',
        'source': source,
    })

with open(os.path.join(out_dir, 'corpus.jsonl'), 'w', encoding='utf-8', newline='\n') as f:
    for r in records:
        f.write(json.dumps(r, ensure_ascii=False) + '\n')

train = [r for i, r in enumerate(records) if i % 5 != 0]
test = [r for i, r in enumerate(records) if i % 5 == 0]
open(os.path.join(out_dir, 'train.txt'), 'w', encoding='utf-8').write('\n'.join(r['id'] for r in train) + '\n')
open(os.path.join(out_dir, 'test.txt'), 'w', encoding='utf-8').write('\n'.join(r['id'] for r in test) + '\n')

readme = f'''# Lykaon corpus

{len(records)} programs that were **executed** with `lykaon-cli run` and whose
output matched the expected text (interpreter, this machine). This is not a
synthetic list: every entry was run.

- `corpus.jsonl` — one JSON object per program: id, task, code, expected,
  difficulty, features, license (CC0-1.0), source.
- `train.txt` / `test.txt` — 80/20 split by index.
- No entry uses Ask, files, network, tasks, databases or other
  non-deterministic/io features; they are deterministic and offline.

Regenerate with `tools/build_corpus.py` (or the script used here); do not hand-edit.
'''
open(os.path.join(out_dir, 'README.md'), 'w', encoding='utf-8', newline='\n').write(readme)
print('candidates:', len(cases), 'passed:', len(records))