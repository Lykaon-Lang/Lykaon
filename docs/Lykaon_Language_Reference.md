# Lykaon Language Reference

**Lykaon is a programming language you read like plain English.**
A program is a list of sentences. A sentence ends with a period. A block is
indented. There are no quotes, braces or semicolons.

```lykaon
Say Hello World.
```

```
Hello World
```

This reference covers the language core, data and ownership, the native
compiler, servers/concurrency, and the data/AI/database/web layer.

---

## 1. How a program reads

- **Sentence** — one line ending with `.`
- **Block** — indented sentences under an opener
- **Name** — letters and digits, e.g. `total`, `price2`
- **Value** — a whole number, decimal, text, yes/no, or missing
- **Comment** — `Note ...` or `About ...`

```lykaon
Note this is a comment.
About double gives its input times 2.
To double n give n times 2.
Say double 21.
```

```
42
```

Text is written as words, not quotes:

```lykaon
Greeting is hello world.
Say Greeting.
```

```
hello world
```

---

## 2. Saying and asking

| Sentence | Meaning |
|---|---|
| `Say hello.` | print `hello` |
| `Say 1 plus 2.` | print `3` |
| `Say total to 2 places.` | print with 2 decimals |
| `Ask name.` | read one line into `name` |
| `Ask age as number.` | read a number |
| `Ask price as decimal.` | read a decimal |

```lykaon
Ask name.
Say hello name.
```

Input `Ada`:

```
hello Ada
```

---

## 3. Names and values

```lykaon
Total is 20.
Changeable score is 10.
Constant limit is 5.
Set score to 12.
Say score.
```

```
12
```

- `is` defines an immutable name.
- `Changeable ... is ...` defines a name you may `Set`.
- `Constant ... is ...` defines a name that may not change.
- `Set x to value.` changes a changeable name.

Kinds are plain English: `whole number`, `decimal`, `text`, `yes or no`,
`list of T`, `map of K to V`, `maybe T`.

---

## 4. Choices

```lykaon
If score is more than 10.
    Say high.
Otherwise if score is more than 5.
    Say middle.
Otherwise.
    Say low.
```

One-line form:

```lykaon
If score is more than 10 say high.
```

Questions: `is`, `is not`, `is more than`, `is less than`, `is at least`,
`is at most`, `is between ... and`, `has`, `is empty`, `is missing`.

---

## 5. Repeating

```lykaon
Repeat 3 times.
    Say tick.
```

```lykaon
Changeable n is 0.
Repeat while n is less than 3.
    Set n to n plus 1.
    Say n.
```

- `Repeat forever.` loops until `Stop.` or `Skip.`
- `Repeat while cond.` loops while the question is yes.
- `Stop.` ends the loop; `Skip.` jumps to the next round.

`For each`:

```lykaon
Nums is list of 1 and 2 and 3.
For each n in Nums.
    Say n.
```

```
1
2
3
```

`For each n from 1 to 3.` counts ranges.

---

## 6. Functions

```lykaon
To double n give n times 2.
Say double 21.
```

```
42
```

Inputs:

```lykaon
To greet name with greeting give greeting plus name.
Say greet with hello and Maya.
```

Functions can fail:

```lykaon
To halve n can fail.
    If n is 0 fail with zero cannot be halved.
    Give back n divided by 2.
```

`Give back` returns a value. `Hand back` yields one item from a generator.

---

## 7. Records and enums

```lykaon
A person has name as text and age as whole number.
Maya is a person with name Maya and age 31.
Say name of Maya.
Say age of Maya.
```

```
Maya
31
```

Enums:

```lykaon
A colour is red or green or blue.
Shade is green.
Look at Shade.
    When red.
        Say warm.
    When green.
        Say fresh.
    When blue.
        Say cool.
```

```
fresh
```

---

## 8. Lists, maps and sets

```lykaon
Nums is list of 1 and 2 and 3.
Say how many in Nums.
Add 4 to Nums.
Say Nums.
```

```
3
1, 2, 3 and 4
```

Maps and sets:

```lykaon
Ages is map of text to whole number.
Put 31 in Ages under Maya.
Say value of Maya in Ages.
```

```lykaon
Tags is set of red and green.
```

---

## 9. Modules

```lykaon
Use helpers.
Share double.
```

- `Use file.` brings another module's shared names in.
- `Share name.` makes a definition visible to other modules.

---

## 10. Errors, fallback and tests

```lykaon
Try halve 0.
    Say it.
If that fails.
    Say could not halve.
```

- `Try <sentence>.` runs a can-fail sentence.
- `If that fails.` handles the failure.
- `Fail with text.` raises a failure inside a can-fail function.
- `maybe T`, `missing`, `fallback` handle absent values.

Tests:

```lykaon
Test double works.
    Check that double 21 is 42.
```

`Check that this fails.` expects a failure. `Pretend f gives V.` overrides a call.

---

## 11. Ownership

Lykaon tracks who owns a value:

- `give` moves it.
- `lend` borrows it.
- `copy of` copies a copy-kind value.
- `share` / `weak share` share or link.
- `Changeable` names can be changed; `Constant` names cannot.

The compiler reports T0401/T0402/T0404 when a value is used after it was moved
or while it is lent.

---

## 12. Grammar at a glance

This is a readable summary, not the raw appendix:

```
program     = repeated top_item
top_item    = note | about | function | record | enum | use | share | test | statement
statement   = simple | block
simple      = define | set | say | ask | add | remove | put | return | if-line | ...
block       = INDENT repeated statement DEDENT

define      = NAME "is" expr
set         = "Set" NAME "to" expr
say         = "Say" expr
ask         = "Ask" NAME ("as" type)?
if          = "If" cond block ("Otherwise if" cond block)* ("Otherwise" block)?
repeat      = "Repeat" (NUMBER "times" | "while" cond | "forever") block
for_each    = "For each" names "in" expr block
function    = "To" name inputs ("can fail")? ("give" expr | block)
record      = "A" NAME "has" field ("and" field)*
enum        = "A" NAME "is" variant ("or" variant)*
```

Types: `whole number`, `decimal`, `text`, `yes or no`, `list of T`, `map of K to V`,
`set of T`, `queue of T`, `maybe T`, `array of N T`, records and enums.

---

## 13. Ambiguity: the postfix rule

Some sentences can be read two ways. Lykaon refuses them instead of guessing.

```lykaon
Say first 2 of numbers sorted.
```

This can mean:

1. take the first 2 of `numbers`, then sort them, or
2. take the first 2 of the sorted `numbers`.

The compiler reports **T0130** and asks you to split the sentence. Postfix words
include: `sorted`, `sorted descending`, `reversed`, `unique`, `trimmed`,
`backwards`, `rounded`, `rounded to even`, `fallback X`, `joined with X`,
`normalized`, `shuffled`, `as <type>`. Two postfix words in a row are **T0109**.

---

## 14. Errors: Where, Why, Fix

Every diagnostic has four parts:

```
Error: <name> (<code>)
Where: <file>, line <N>: <sentence>
  ^^^^
Why: <what went wrong>
Fix: <what to write instead>
```

Common codes:

| Code | Meaning |
|---|---|
| T0110 | No action word — the sentence does not start with an action |
| T0113 | A sentence cannot be read — check the wording |
| T0205 | A name is not defined |
| T0301 | A value of one kind is used where another is needed |
| T0406 | A fixed name was changed |
| T0509 | A sentence belongs to a later version |
| T0611 / T0612 | Memory / time limit reached |
| T0622 | Sandbox refused |

The compiler reports one main error per mistake, with the Where line marked.

---

## 15. Data, AI, databases and tools

### Data tables

```lykaon
Load sales.
Say rows.
Say columns.
Say average price.
Say total price by region.
Say top 5 rows by price.
For each row in sales.
    Say price.
Save sales as csv to file sales-out.
```

`Load` reads CSV from the `data` folder (or the file's folder). Kinds are
inferred from the values.

### Charts

```lykaon
Plot price against size.
Plot price against size as dots to file chart.
Plot region against price as bars to file chart.
Plot price against size as line to file chart with extension png.
```

The default is an 800×480 SVG in `output/plot-N.svg`; `with extension png`
writes a deterministic PNG.

### Models

```lykaon
Predict churned from age and plan.
Say accuracy.
Say error.
Say what matters most.
Say prediction for age 40 and plan basic.
Save model as churn.
Load model churn.
```

### Databases

```lykaon
Open database shop using sqlite file shopdata.
Open database pg using postgres at localhost on port 5432 path shop.
Load stock from database shop.
Add a stockitem with name apples and count 5 to table stock of shop.
Remove rows where name is apples from table stock of shop.
Set count to 6 in rows of stock of shop where name is pears.
Do these all or none.
    Add a stockitem with name plums and count 2 to table stock of shop.
```

`lykaon migrate` applies SQL files in `migrations/` and records them in
`lykaon_migrations`.

### Encryption, limits and sandbox

```lykaon
Key is random key.
Secret is encrypted 42 with key.
Say decrypted secret with key.
Limit memory 64 megabytes.
Limit time 10 seconds.
```

`lykaon run safely` reads `Allows files ... and network ... and environment
... and processes.` from `settings.lykaon` and enforces it.

### Tools

`lykaon lsp`, `lykaon doc`, `lykaon profile`, `lykaon outline`,
`lykaon check --json`, `lykaon test --json`, `lykaon docs --compact`,
`lykaon explain`, `lykaon run --trace-json`, `lykaon migrate`.

### Status and limits

- The language core, data and ownership, native compilation, servers,
  concurrency, files, formats and networking are implemented.
- Data tables, SVG + PNG charts, classical ML, SQLite/PostgreSQL (MD5 auth;
  real-server Add/Remove/Set/Load/transactions), encryption, limits, the
  native permission sandbox, `lykaon lsp`, `lykaon doc`, `lykaon profile`,
  `lykaon outline`, JSON output modes and `lykaon migrate` are implemented.
- **WebAssembly**: the browser playground runs the interpreter compiled to
  wasm32; native Lykaon-to-wasm output and the WASI sandbox are deferred.
- **PostgreSQL SCRAM-SHA-256** is not implemented yet; MD5 auth works.
- **Windows AppContainer** and **wasm32-wasi** isolation are deferred.

Lykaon is **source-available, not open source**: free for personal use and for
organizations under USD 1,000,000 annual revenue. Programs you write in Lykaon
are yours to license and sell.