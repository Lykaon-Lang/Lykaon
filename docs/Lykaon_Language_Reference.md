# Lykaon Language Reference — Version 0.1 (M1–M4)

*Complete Build Reference for Lykaon v0.1. Normalized for the Lykaon name
throughout. Every code example in this reference is taken from the language's
conformance test suite and runs on the unmodified compiler.*

---

## 1. About this reference

Lykaon is a programming language written in plain English sentences.

- A **sentence** ends with a period (`.`).
- A **block** is the indented sentences under an opener sentence.
- There are **no quoted strings**: words in code are raw text. Quote marks are
  refused with `Error: Quote in code. (T0008)`.
- Comments and documentation are `Note` and `About` sentences.
- Names are letters and digits (e.g. `score`, `Number2`).

Version 0.1 covers milestones M1–M4: language core, data and ownership, native
compilation, and servers/concurrency. Windowed (GUI) apps arrive in a later
version; v0.1 programs are command-line and console executables.

---

## 2. Names and values

```lykaon
Say Hello World.
```

| Sentence | Meaning |
|---|---|
| `Changeable x is 10.` | introduce a name you may re-set |
| `Constant limit is 5.` | introduce a name that may not change |
| `Total is 20.` | plain immutable definition |
| `Set x to 3.` | give a changeable name a new value |
| `Ask name.` | read one line from the keyboard into `name` |
| `Say hello.` | print (one sentence may print several values) |

Block printing:

```lykaon
Say lines.
    first line
    second line
```

- `it` means *the last result*; `them` means *the last list*.
- `Ask` fails with *no person here* when stdin is not a terminal
  (use `For each line in input.` for piped input).

### Kinds

| Kind words | Holds |
|---|---|
| `whole number` | integer (plain numbers are whole numbers) |
| `number` | whole or decimal |
| `decimal` | decimal number |
| `text` | words |
| `yes or no` | true/false |
| `maybe T` | a value of kind T or `missing` |
| `list of T`, `map of K to V`, `set of T`, `pair of A and B` | collections |
| `channel of T` | bounded channel (optional `holding N`) |
| `guarded`, `atomic number`, `counter` | shared values |
| `date`, `moment`, `duration`, `secret` | time/secret values |

Kinds may be given at definition: `Phone is missing as maybe number.`

---

## 3. Numbers, text and comparisons

Operations: `plus`, `minus`, `times`, `divided by`, `whole divided by`,
`rem`, `pow`. Rounding: `rounded`, `rounded to even`. Conversion: `as decimal`.

```lykaon
Say 2.5 rounded.
Say 2.5 rounded to even.
```

Comparisons (used with `If`, `Repeat while`, `where`, contracts):

```
is, is not, is more than, is less than, is at least, is at most
```

Compound conditions join with `and` and `or`. Text is grapheme-aware;
`position of x in y`, `how many in x`. Text ordering is by Unicode code point.

---

## 4. Collections

From the conformance program `collections.lykaon`:

```lykaon
Numbers is list of 5 and 2 and 8.
Say item 2 of numbers.
Say first 2 of numbers.
Say last of numbers.
Say items 2 to 3 of numbers.
Say how many in numbers.
Say if numbers is empty.
Spot is position of 8 in numbers.
Say spot fallback 0.
Changeable nums is list of 5 and 2 and 8.
Add 9 to nums.
Remove 2 from nums.
Say nums.
Say numbers sorted.
Say numbers sorted descending.
Say numbers reversed.
Say 3 and 3 and 4 unique.
```

Maps, sets and pairs:

```lykaon
Changeable ages is map of text to number.
Put 31 in ages under Maya.
Put 27 in ages under Omar.
Say how many in ages.
Found is value of Maya in ages.
Say found fallback 0.
Say if ages has Maya.
For each name and age in ages.
    Say name age.
Remove Maya from ages.
Changeable tags is set of text.
Put red in tags.
Put blue in tags.
Put red in tags.
Say how many in tags.
Changeable left is set of number.
Put 1 in left.
Put 2 in left.
Put 3 in left.
Changeable right is set of number.
Put 3 in right.
Put 4 in right.
Say union of left and right.
Say overlap of left and right.
Say difference of left and right.
Pair is pair of 1 and 2.
Say first of pair.
Say second of pair.
```

- `Numbers where it is more than 3.` filters a list
- Mutable collections must be introduced with `Changeable`

---

## 5. Records, enums and `Look at`

From the conformance programs `record.lykaon`, `color.lykaon`, `pay.lykaon`:

```lykaon
A person has name as text and age as number.
Maya is a person with name Maya and age 31.
Say name of maya.
Say age of maya.

A color is red or green or blue.
Shade is green.
Look at shade.
    When red.
        Say warm.
    When green.
        Say fresh.
    When blue.
        Say cool.

A payment is cash or card with owner as text.
Pay is card with owner Maya.
Look at pay.
    When cash.
        Say paid in cash.
    When card owner.
        Say paid by card owner.
```

`Look at` must cover every enum variant. A `When` on a record variant can name
a part (`When card owner.`). Records that `can compare` work with `Less`/`More`/
`AtLeast`/`AtMost`.

---

## 6. maybe, missing, failure

From the conformance programs `maybe.lykaon` and `try_fail.lykaon`:

```lykaon
Phone is missing as maybe number.
If phone is missing say no phone.
If phone is there say has phone.
Say phone fallback 0.

To halve n can fail.
    If n is 0 fail with zero cannot be halved.
    Give back n divided by 2.
Try halve 0.
    Say it.
If that fails.
    Say could not halve.
```

Checked and saturating math variants exist so overflow is an event, not a wrap.

---

## 7. Control flow

```lykaon
If age is at least 18 and age is at most 65.
    Say working age.
Otherwise if age is at least 13.
    Say teen.
Otherwise.
    Say child.

Repeat 3 times say hi.
Repeat 3 times.
    Say round.
Repeat forever.
    ...
Repeat while count is at most 3.
    ...
For each n in numbers.
    Say n.
For each i from 1 to 5.
    Say i.
Changeable ages is map of text to number.
Put 31 in ages under Maya.
For each name and age in ages.
    Say name age.
For each line in file list in temporary folder.
    Say line.
For each x as they arrive from bag.
    Say x.

Stop.
Skip.
```

`Stop` leaves the loop/round/scope; `Skip` starts the next round.

---

## 8. Functions

```lykaon
To head items give item 1 of items.
Numbers is list of 5 and 2 and 8.
Say head Numbers.

To sumto n.
    Changeable total is 0.
    For each i from 1 to n.
        Set total to total plus i.
    Give back total.
Say sumto 100.

To pad lend to change items.
    Add 0 to items.
    Give back how many in items.
Changeable numbers is list of 5 and 2.
Say pad numbers.
```

- One-line bodies: `To head items give item 1 of items.`
- Interfaces pass the receiver as an input (`To speak pet give woof.`)
- `give` moves, `lend` borrows, `copy of x` copies, `share` shares, `weak` weakens
- A kind may define `can clean up.` with `Which is ...`; it runs when a value
  goes away (conformance program `cleanup`)

### Closures, Apply, Combine

Local `To` functions capture their surroundings at definition.
`Apply` calls a function once per list item; `Combine` folds a list with
`plus`, `minus` or `times`.

### Generators

From the conformance program `generator`:

```lykaon
To squares n
    For each i from 1 to n.
        Hand back i times i.

Say first 3 of squares 5.
```

`Hand back` suspends the function and yields one value per pull; body locals
persist between pulls. Built-ins like `first N of` are truly lazy.

---

## 9. Interfaces and dispatch

From the conformance program `interfaces`:

```lykaon
Anything that can speak must have speak.

A dog has name as text.
A dog can speak.
    To speak pet give woof.

A cat has name as text.
A cat can speak.
    To speak pet give meow.

Rex is a dog with name Rex.
Tom is a cat with name Tom.
Speakers is list of Rex and Tom.
For each s in Speakers.
    Say speak s.
```

- `Anything that can VERB must have VERB.` declares an interface
- `A KIND can VERB.` implements it; calls dispatch on the value's kind
- Static dispatch is devirtualized when the receiver's kind is known

---

## 10. Contracts

From the conformance program `t30`:

```lykaon
To halve n.
    Requires n is more than 0.
    Ensures result is more than 0.
    Give back n divided by 2.
Say halve 8.
```

- `Requires` is checked at every call (never skipped)
- `Ensures` is checked on return; the contract prover removes checks it proves
- `Always` on a kind is checked at construction and after a part is set
  (violation: `Error: Always failed: <cond>. (T0602)`, exit 1)
- `Tagged` marks kinds intentionally crossing safety boundaries
- Unprovable results fail `T0603` instead of producing a value

---

## 11. Tests

From the conformance test programs:

```lykaon
To double n give n times 2.
Test double works.
    Check that double 21 is 42.
    Check that double 0 is 0.

Test adding zero changes nothing.
    Check for any number n.
        Check that n plus 0 is n.

To halve n can fail.
    If n is 0.
        Fail with zero.
    Give back n whole divided by 2.
Test halving zero fails.
    Check that this fails.
        Say halve 0.
```

- `Test NAME.` opens a test; `Check that <a> is <b>.` asserts
- `Check for any number n.` runs a property over generated values
- `lykaon-cli test file.lykaon` runs them; each test runs on a fresh frame

---

## 12. Modules and packages

From the conformance program `modules`:

`helper.lykaon`:

```lykaon
Share double.
To double n give n times 2.
```

`input.lykaon`:

```lykaon
Use helper.
Say double 21.
```

- `Use NAME.` loads the file `NAME.lykaon` (or a package dependency)
- `Share NAME.` exposes a top-level name to users of the module
- `settings.lykaon` next to the program is read into top-level names
  automatically (`Use settings.` also loads a settings module)

`Lykaon.toml` (manifest, sentence format):

```lykaon
Name is myapp.
Version is 1 point 0 point 0.
Needs helpers from folder helpers.
Needs tools from git https://example.com/tools.git at version 1 point 2 point 0.
```

Commands: `lykaon add/remove/update` — see `cli.md`. Git deps are pinned by
tag and verified by a SHA-256 tree checksum in `lykaon.lock`.

---

## 13. Tasks and concurrency

From `tests/m4.rs`:

```lykaon
Start task looker.
    Give back 11.
Wait for looker.
Say it.

Do these together.
    Start task maker.
        Give back 11.
Wait for maker.
Say it.

Start task giver.
    Give back 12.
Wait for giver.
Say it.

Stop task looker.
```

- `Start task NAME.` starts the block as a task and goes on
- `Wait for NAME.` gives the result to `it` (move kinds are taken once;
  a second wait on a taken result is `T0334`)
- `Do these together.` runs its task starts concurrently; the task names stay
  waitable after the block
- `Stop task NAME.` cancels; a finished task is a harmless no-op
- Task capture is checked: a task may not carry a plain move-kind value from
  outside in (`T0450`); a `Changeable` may not be changed from a task
  (`T0451`); a task handle kept past its block is `T0462`

Channels:

```lykaon
Bag is channel of whole number.
Send 1 to bag.
Send 2 to bag.
Send 3 to bag.
Changeable total is 0.
For each x as they arrive from bag.
    Set total to total plus x.
Say total.
```

- `Bag is channel of whole number.` (optionally `holding N`, capacity 1 to
  1,000,000); `Send 5 to bag.` puts a value in; `Receive from bag.` takes
  one out; `For each x as they arrive from bag.` drains in order
- Values are moved (move kinds) or copied (copy kinds) across the queue

Guarded, atomic, counter:

```lykaon
Tally is guarded 0.
Set tally to tally plus 1.
Say tally.

Hits is counter.
Add 1 to hits.
Say hits.

N is atomic number.
Add 5 to N atomically.
Say N.
```

- `guarded` — a fixed name changed through `Set`
- `counter` — starts at 0, changed with `Add` (`Set` is refused, T0406)
- `atomic number` — changed with `Add ... atomically` (one step)

Workers and Spread:

```lykaon
Nums is list of 1 and 2.
Use 4 workers.
Changeable total is 0.
Spread each of nums over workers.
    Give back it times 3.
For each r in them.
    Set total to total plus r.
Say total.
```

- `Use 4 workers.` (1–256) sets the worker count for the `Spread` sentences
  that follow in the block
- `Spread each of NUMS over workers.` runs one round per item; the item is
  `it`; results collect in `them` in item order; a failing round stops the rest

---

## 14. Timers

From the timer tests:

```lykaon
Hits is counter.
Every 30 milliseconds.
    Add 1 to hits.
Sleep for 100 milliseconds.
Say hits.

Hits is counter.
Every 20 milliseconds.
    Add 1 to hits.
    If hits is 2.
        Stop.
Sleep for 200 milliseconds.
Say hits.

After 40 milliseconds.
    Say time is up.
```

- `After <duration>.` with a block fires once, at its deadline
- `Every <duration>.` with a block repeats at a fixed rate (rounds never
  overlap); `Stop.` inside a round ends the `Every`
- The program stays alive while an `After` is pending in its main scope
- Durations: `milliseconds`, `seconds`, `minutes`, `hours`, `days` (plural)

---

## 15. Files, folders, zip

From the file tests:

```lykaon
Write 42 to file score in temporary folder.
Say Read file score in temporary folder.
Append 7 to file log in temporary folder.
Replace file data in temporary folder with 2.
For each line in file list in temporary folder.
    Say line.
Make folder backup in temporary folder.
Remove folder backup in temporary folder.
Remove file old in temporary folder.
Move file a in temporary folder to file b in temporary folder.
Duplicate file b in temporary folder as file c in temporary folder.
List files in folder backup in temporary folder.
If file b in temporary folder exists.
    Say there.
Make temporary file scratch.
When file watch changes.
    Say it changed.
Zip folder backup in temporary folder to file pack in temporary folder.
Unzip file pack in temporary folder to folder out in temporary folder.
Gzip file big in temporary folder to file gz in temporary folder.
```

- Roots: `in temporary folder` (OS temp dir), `in folder X` (chained steps);
  a bare name resolves next to the program file
- `Write` refuses to overwrite an existing non-temporary file;
  `Replace` is atomic (sibling temp file + one rename)
- `If file F exists.` is a yes-or-no question; `Make temporary file NAME.`
  binds a fresh temporary file (removed at exit)
- `Read` keeps line endings; `For each line` streams lines without endings
- `List files in` returns names sorted; `Zip` writes deflate archives with
  sorted entries and a fixed stamp

---

## 16. Formats: JSON, TOML, YAML, XML, CSV

From the format tests:

```lykaon
A creature has legs as number and eyes as number.
Save a creature with legs 3 and eyes 1 as json to file c in temporary folder.
Read json file c in temporary folder as creature.
Save 42 as json to file n in temporary folder.
Read json file n in temporary folder as whole number.
Say json of list of 1 and 2.
```

- `Read <fmt> file F as KIND.` / `Read <fmt> from text V as KIND.` /
  `Save V as <fmt> to file F.`
- Format words: `json`, `toml`, `yaml`, `xml`, `csv`
- Decoding is kind-directed: record parts by name (extra keys ignored),
  lists from arrays, maps from objects (in order), enums as
  `{"kind": variant}`; depth limit 256
- `json of x` is compact; `Save ... as json` is pretty with a final line feed
- CSV needs a header row covering all record parts

---

## 17. Time, random, hash, secret

```lykaon
Use seed 42.
Say today.
Say random number from 1 to 10.
Changeable m is moment of today at 14 and 30.
Changeable u is now in zone utc.
Say hash of hello.
Key is secret hunter2.
Say reveal key.
```

- Kinds: `date`, `moment`, `duration`, `secret`
- `today`/`now` are local-zone; `in zone` converts (zone names are written in
  lower-case words, e.g. `asia dhaka` or `utc`; zones are bundled; an unknown
  zone is refused at compile time, T0171)
- `moment of D at H and M` builds a moment; impossible dates are `T0620`
- `Use seed N.` makes random reproducible (Fisher–Yates for `shuffled`)
- `hash of` is SHA-256, 64 lower-case hex characters
- A `secret` never prints its text (a `Log` of one writes `hidden`), cannot be
  compared or converted; open it with `reveal`

---

## 18. System: Run, Exit, signals, environment, CLI tools

```lykaon
Exit with 0.
When signal interrupt arrives.
    Say stop requested.
Say Environment HOME as maybe text.
```

- `Run m48sayhi with first and second.` runs a program by name with arguments
  (no shell); its stdout is `it` (the program is found next to your program
  file); a nonzero child exit is a kind-other failure carrying the child's
  stderr
- `Exit with N` (0–255) ends the program after cleanups unwind
- Signals: `When signal interrupt/stop/hangup arrives.` with an indented
  block; the poll happens after each sentence; a handlerless interrupt exits
  130, stop 143, hangup 129
- `Environment NAME as maybe text` reads an environment variable (name is
  upper-cased with `_` for spaces)

`Takes`/`Option` sentences (before every other sentence except `Note`/`About`)
make the file a command-line tool:

```lykaon
Takes file name as text.
Option verbose is whole number.
Option log level is text defaulting to debug.
```

- positional arguments bind to `Takes` in order; `--name value` fills an
  `Option`; a bare `--flag` is the word `on` for text options
- `--help` first prints usage and exits 0; a missing `Takes` argument or bad
  usage prints usage and exits 2
- `Read input.` reads one line from standard input; `For each line in input.`
  streams stdin (empty stdin is zero rounds)

---

## 19. Logging

```lykaon
Use log level debug.
Log info server started.
Log warning hot.
```

- Levels `debug`, `info`, `warning`, `error`; default `info`; logs go to
  standard error, program output to standard output, merged in order
- The log text is rendered exactly like `Say` (names, secrets hidden,
  counters shown)
- `Use log level X.` sets the floor; lower levels are not written

---

## 20. Network: Fetch, Serve, WebSocket

Addresses are written with words: `127 dot 0 dot 0 dot 1`, `example dot com`.
From the network tests:

```lykaon
Reply is Fetch plain 127 dot 0 dot 0 dot 1 on port 8080 home.
Say status of reply.
Say body of reply.
Posted is Post hello to plain 127 dot 0 dot 0 dot 1 on port 8080 home.

Serve on port 8080.
    When someone gets home.
        Respond with status 200 and text hello.
```

- `Fetch` gives `{status, body, headers}`; status 400+ is a response, not a
  failure; requests retry until a deadline (sent-once rule)
- `Post x to <address> <page>.` posts data; `Fetch plain example dot com
  without checking certificates.` skips TLS checking (only inside `Be careful.`)
- `Serve securely on port 8443 using certificate file C and key file K.` — TLS
- Pages: `When someone gets <page> page.` (also `posts`); inside, `method`,
  `page`, `text`, `data`, `header`, `query` and `parameter <name>` are available
- Every path must reach exactly one `Respond with` (`T0166` otherwise)
- Answers: `Respond with <text>.` (200), `Respond with status 200 and text
  <text>.`, `Respond with json x.` (200, application/json), `Respond with
  status N.`, `Respond with status N and text <text>.`
- Sessions: `Put hello in session under name.` inside a handler
  (30-minute expiry, HttpOnly SameSite=Lax cookie)
- WebSocket: `When someone opens <page> page.`; inside, `connection` is a
  first-class value; `Send welcome to connection.` sends a text frame;
  `Receive from connection.` gives maybe-text (missing on close)
- Port already in use: `T0618`; a page pattern must be unique (`T0172`);
  fetching an undeclared page is `T0173`

---

## 21. Diagnostics

Errors follow one shape:

```
Error: <message>. (T<code>)
Where: <file>, line N: <the sentence>
Why: <plain-English reason>
Fix: <suggested correction>
```

Common codes in v0.1:

| Code | Meaning |
|---|---|
| T0007 | a symbol such as a bracket or operator in code |
| T0008 | a quote mark in code (Lykaon has no quoted text) |
| T0113 | the sentence does not fit any known shape |
| T0120 | two names of the same length both fit a word run (ambiguous) |
| T0130 | a two-reading (ambiguous) sentence |
| T0151 | `Try` around a sentence that cannot fail |
| T0158 | unknown dynamic kind (interface names no interface) |
| T0164 | `Limit` names no interface |
| T0166 | a Serve page path reaches no `Respond with` |
| T0168 | waiting/stopping an unknown task name |
| T0171 | unknown spelling (word/zone/program name) |
| T0172 | duplicate page pattern / missing template |
| T0173 | fetching an undeclared page |
| T0174 | `Takes`/`Option` after other sentences |
| T0205 | assignment to a name that is not changeable |
| T0207 | duplicate temporary file name |
| T0209 | a `Use` that is not a module Use |
| T0308 | map key kind differs (typed decode) |
| T0309 | record part kind differs (typed decode) |
| T0323 | kind cannot do that (e.g. dynamic kind mismatch) |
| T0324 | kind cannot compare (limit semantics) |
| T0334 | second wait on a taken move result |
| T0406 | shared-value misuse at `Set` |
| T0450 | a value crosses tasks unsafely |
| T0451 | a task changes a captured `Changeable` |
| T0460/T0462 | task cancellation / task outlives its block |
| T0515 | a folder dependency that is not next to the project |
| T0520 | git dependency failed (address, tag, checksum) |
| T0602 | `Always` failed |
| T0603 | a result left the whole-number domain (contract) |
| T0607 | call depth limit |
| T0618 | port in use |
| T0620 | impossible date |

---

## 22. Known limits in version 0.1

- Windows x86-64 is fully tested. Linux x86-64 is built and smoke-tested on
  every push by CI (ubuntu-latest); the full stress suite has not been run on
  Linux hardware yet. macOS is not a target in v0.1.
- Roughly 67–72% of equivalent Rust speed on the six standard benchmarks.
- Windowed (GUI) apps are a later milestone; v0.1 builds command-line and
  console executables only.
- `Every` without a `Sleep for` fires nothing in v0.1.
- `can add` at run time is not implemented in v0.1.
- Some constructs (closures, Apply/Combine, dynamic dispatch, generators)
  evaluate through the embedded evaluator inside native builds; output is
  byte-identical to the interpreter by design and test.