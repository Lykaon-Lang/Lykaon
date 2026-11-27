# Lykaon corpus

1000 programs that were **executed** with `lykaon-cli run` and whose
output matched the expected text (interpreter, this machine). This is not a
synthetic list: every entry was run.

- `corpus.jsonl` - one JSON object per program: id, task, code, expected,
  difficulty, features, license (CC0-1.0), source.
- `train.txt` / `test.txt` - 80/20 split by index.
- No entry uses Ask, files, network, tasks, databases or other
  non-deterministic/io features; they are deterministic and offline.

Regenerate with `tools/build_corpus.py` then `tools/corpus/gen_more.py`.
