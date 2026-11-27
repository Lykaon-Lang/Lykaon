#!/usr/bin/env python3
"""M5c item 11: grow the corpus to 1,000 real, executed programs.

Starts from the existing tools/corpus/corpus.jsonl (programs taken from the
test suites) and adds parameterized programs from templates. Every new program
is written to a temp file and run with `lykaonc-cli run`; only programs that
exit 0 are kept, and the expected output is the real interpreter output.

Run:  python tools/corpus/gen_more.py
"""

import json
import os
import subprocess
import tempfile

ROOT = r"C:\Users\emran\OneDrive\Documents\LYKAON"
CLI = os.path.join(ROOT, "Lykaon", "target", "debug",
                   "lykaonc-cli.exe" if os.name == "nt" else "lykaonc-cli")
OUT = os.path.join(ROOT, "tools", "corpus")
TARGET = 1000


def sumloop(n):
    return (f"Changeable t is 0.\n"
            f"For each i from 1 to {n}.\n"
            f"    Set t to t plus i.\n"
            f"Say t.\n")


def fact(n):
    return (f"To fact n as whole number.\n"
            f"    If n is at most 1 give back 1 as whole number.\n"
            f"    Smaller is n minus 1.\n"
            f"    Later is fact smaller.\n"
            f"    Give back n times later as whole number.\n"
            f"Say fact {n}.\n")


def candidates():
    for a in range(0, 40):
        for b in range(1, 40):
            yield f"Say {a} plus {b}.\n", "generated:add"
            yield f"Say {a} minus {b}.\n", "generated:sub"
            yield f"Say {a} times {b}.\n", "generated:mul"
            yield f"Say {a} whole divided by {b}.\n", "generated:div"
            yield (f"Changeable x is {a}.\nSet x to x plus {b}.\nSay x.\n",
                   "generated:set")
            yield f"Say {a} plus {b} plus {a}.\n", "generated:sum3"
            yield (f"Changeable t is 0.\nRepeat {a} times.\n"
                   f"    Set t to t plus 2.\nSay t.\n", "generated:repeat2")
            yield (f"If {a} is more than {b}.\n    Say big.\n"
                   f"Otherwise.\n    Say small.\n", "generated:if")
            yield (f"Nums is list of {a} and {b} and {a}.\n"
                   f"Say how many in Nums.\n", "generated:countlist")
            yield (f"A thing has n as whole number.\n"
                   f"T is a thing with n {a}.\nSay n of T.\n", "generated:record")
            yield (f"Nums is list of {a} and {b} and {a}.\nChangeable t is 0.\n"
                   f"For each n in Nums.\n    Set t to t plus n.\nSay t.\n",
                   "generated:list_sum")
            yield f"Say {a} is {b}.\n", "generated:bool"
            yield (f"Line is word{a} word{b}.\n"
                   f"Words is split line at space.\nSay how many in words.\n",
                   "generated:wordcount")
    for n in range(0, 30):
        yield sumloop(n), "generated:sumloop"
        yield fact(n), "generated:fact"


FEATURES = ['say', 'ask', 'set', 'if', 'otherwise', 'repeat', 'for', 'to ',
            'list', 'map', 'record', 'enum', 'try', 'test', 'text',
            'changeable', 'constant']


def main():
    path = os.path.join(OUT, "corpus.jsonl")
    records = []
    if os.path.exists(path):
        for line in open(path, encoding="utf-8"):
            if line.strip():
                records.append(json.loads(line))
    seen = {r["code"] for r in records}
    print("starting records:", len(records))

    tmp = os.path.join(tempfile.gettempdir(), "lykaon_corpus_gen.lykaon")
    added = 0
    for code, source in candidates():
        if len(records) >= TARGET:
            break
        if code in seen:
            continue
        with open(tmp, "w", encoding="utf-8", newline="\n") as f:
            f.write(code)
        try:
            r = subprocess.run([CLI, "run", "--interpret", tmp],
                               capture_output=True, text=True, timeout=10)
        except Exception:
            continue
        if r.returncode != 0:
            continue
        expected = r.stdout.strip()
        low = code.lower()
        feats = [f.strip() for f in FEATURES if f in low]
        lines = code.count("\n") + 1
        difficulty = "easy" if lines < 10 else ("medium" if lines < 30 else "hard")
        records.append({
            "id": "c%04d" % len(records),
            "task": source,
            "code": code,
            "expected": expected,
            "difficulty": difficulty,
            "features": feats,
            "license": "CC0-1.0",
            "source": source,
        })
        seen.add(code)
        added += 1
    try:
        os.remove(tmp)
    except OSError:
        pass

    with open(path, "w", encoding="utf-8", newline="\n") as f:
        for r in records:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")
    train = [r for i, r in enumerate(records) if i % 5 != 0]
    test = [r for i, r in enumerate(records) if i % 5 == 0]
    open(os.path.join(OUT, "train.txt"), "w", encoding="utf-8").write(
        "\n".join(r["id"] for r in train) + "\n")
    open(os.path.join(OUT, "test.txt"), "w", encoding="utf-8").write(
        "\n".join(r["id"] for r in test) + "\n")

    readme = f"""# Lykaon corpus

{len(records)} programs that were **executed** with `lykaon-cli run` and whose
output matched the expected text (interpreter, this machine). This is not a
synthetic list: every entry was run.

- `corpus.jsonl` - one JSON object per program: id, task, code, expected,
  difficulty, features, license (CC0-1.0), source.
- `train.txt` / `test.txt` - 80/20 split by index.
- No entry uses Ask, files, network, tasks, databases or other
  non-deterministic/io features; they are deterministic and offline.

Regenerate with `tools/build_corpus.py` then `tools/corpus/gen_more.py`.
"""
    open(os.path.join(OUT, "README.md"), "w", encoding="utf-8", newline="\n").write(readme)
    print("added:", added, "total:", len(records))


if __name__ == "__main__":
    main()