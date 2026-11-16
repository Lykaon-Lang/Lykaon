# The Lykaon standard library (v0.1)

Everything in this list is **built in** — there is nothing to import for it. File
modules loaded with `Use` add your own names on top. Words are grouped by task.

Every code block below is taken from the language's conformance test suite
and runs on the unmodified compiler.

## Numbers and text

```lykaon
Say 2.5 rounded.
Say 2.5 rounded to even.
```

- The four operations: `plus`, `minus`, `times`, `divided by`
- Whole-number forms: `whole divided by`, `rem`, `pow`
- Conversion: `as decimal`
- Text: `position of x in y` (with `fallback`), `how many in x`,
  `x sorted`, `x sorted descending`, `x reversed`, `x unique`
- Text ordering is by Unicode code point

## Collections

From the conformance program `collections.lykaon`:

```lykaon
Numbers is list of 5 and 2 and 8.
Say numbers.
Names is empty list of text.
Say if names is empty.
Say item 2 of numbers.
Say first 2 of numbers.
Say last of numbers.
Say items 2 to 3 of numbers.
Say how many in numbers.
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

Maps, sets and pairs (from the same program):

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
Say how many in ages.
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

## Records and enums

From the conformance programs `record.lykaon` and `color.lykaon`:

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
```

`Look at` must cover every enum variant. A `When` on a record variant can name a
part (`When card owner.`), as in the conformance program `pay.lykaon`:

```lykaon
A payment is cash or card with owner as text.
Pay is card with owner Maya.
Look at pay.
    When cash.
        Say paid in cash.
    When card owner.
        Say paid by card owner.
```

## maybe, failure and fallback

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

## Ownership

From the conformance program `ownership.lykaon`:

```lykaon
To pad lend to change items.
    Add 0 to items.
    Give back how many in items.
Changeable numbers is list of 5 and 2.
Say pad numbers.
Say numbers.
```

- `give` (move), `lend` (borrow), `copy of x` (copy), `share` (shared handle), `weak`
- A kind may define `can clean up.` with `Which is ...`; it runs when a value goes away
  (conformance program `cleanup`)

## Contracts

From the conformance program `t30`:

```lykaon
To halve n.
    Requires n is more than 0.
    Ensures result is more than 0.
    Give back n divided by 2.
Say halve 8.
```

- `Requires` is checked at every call
- `Ensures` is checked on return; the contract prover removes checks it proves
- `Always` on a kind is checked at construction and after a part is set
  (violation: `Error: Always failed: <cond>. (T0602)`, exit 1)

## Tests

From the conformance test programs:

```lykaon
To double n give n times 2.
Test double works.
    Check that double 21 is 42.
    Check that double 0 is 0.

Test adding zero changes nothing.
    Check for any number n.
        Check that n plus 0 is n.
```

Run them with `lykaon-cli test file.lykaon` (see [`cli.md`](cli.md)).

## Tasks and concurrency

Start a task, wait for its result (from `tests/m4.rs`):

```lykaon
Start task looker.
    Give back 11.
Wait for looker.
Say it.
```

Run tasks together; the task name stays waitable after the block:

```lykaon
Do these together.
    Start task maker.
        Give back 11.
Wait for maker.
Say it.
```

- `Start task NAME.` runs the block as a task and goes on
- `Wait for NAME.` gives the result to `it` (a move-kind result is taken once)
- `Stop task NAME.` cancels; `Sleep for 100 milliseconds.` pauses

Channels (from the stream-loop test):

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

- A channel is declared `Bag is channel of whole number.` (optionally
  `holding N` for a capacity, 1 to 1,000,000)
- `Send 5 to bag.` puts a value in; `Receive from bag.` takes one out
- `For each x as they arrive from bag.` drains the channel in order

Shared values:

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
- `counter` — starts at 0, changed with `Add` (`Set` on a counter is refused, T0406)
- `atomic number` — changed with `Add ... atomically` (one step)

Workers and Spread (from the Spread test):

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

- `Use 4 workers.` sets the worker count (1 to 256) for the `Spread` sentences
  that follow in the block
- `Spread each of NUMS over workers.` runs one round per item; the item is `it`;
  results collect in `them` in item order

## Timers

From the timer tests:

```lykaon
Hits is counter.
Every 30 milliseconds.
    Add 1 to hits.
Sleep for 100 milliseconds.
Say hits.
```

- `After 40 milliseconds.` with an indented block fires once
- `Every 30 milliseconds.` with a block repeats at a fixed rate (rounds never overlap)
- `Stop.` inside a round ends the `Every`
- Durations: `milliseconds`, `seconds`, `minutes`, `hours`, `days` (plural forms)

## Files and folders

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

- `in temporary folder` uses the OS temp dir; `in folder X` chains path steps;
  a bare file name resolves next to the program file
- `Write` refuses to overwrite an existing non-temporary file;
  `Replace` is atomic (a sibling temp file and one rename)
- `If file F exists.` is a yes-or-no question (no comparison needed)
- `Make temporary file NAME.` binds a fresh temporary file (removed at exit)
- `When file F changes.` runs its block when the file changes

## Formats: JSON, TOML, YAML, XML, CSV

From the format tests:

```lykaon
A creature has legs as number and eyes as number.
Save a creature with legs 3 and eyes 1 as json to file c in temporary folder.
Read json file c in temporary folder as creature.
Save 42 as json to file n in temporary folder.
Read json file n in temporary folder as whole number.
Say json of list of 1 and 2.
```

- Format words: `json`, `toml`, `yaml`, `xml`, `csv` — in
  `Read <fmt> file F as KIND.`, `Read <fmt> from text V as KIND.` and
  `Save V as <fmt> to file F.`
- Decoding is kind-directed: record parts by name (extra keys ignored),
  lists from arrays, maps from objects (in order)
- `json of x` gives compact JSON text; `Save ... as json` writes pretty JSON
- CSV reads `list of <record kind>` with a header row

## Time, random, hash, secret

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
- `Use seed N.` makes random reproducible
- `moment of D at H and M` builds a moment; `in zone` converts
  (zone names are written in lower-case words, e.g. `asia dhaka` or `utc`;
  zones are bundled; an unknown zone is refused at compile time, T0171)
- `hash of` is SHA-256, 64 lower-case hexadecimal characters
- A `secret` never prints its text (a `Log` of one writes `hidden`); open it
  with `reveal`

## System: Run, Exit, signals, environment, CLI tools

```lykaon
Exit with 0.
When signal interrupt arrives.
    Say stop requested.
Say Environment HOME as maybe text.
```

- `Run m48sayhi with first and second.` runs a program by name with arguments;
  its stdout is `it` (the program is found next to your program file)
- `Exit with N` (0–255) ends the program after cleanups unwind
- `When signal interrupt arrives.` with an indented block registers a handler
  (interrupt, stop, hangup)
- `Environment HOME as maybe text` reads an environment variable

A file that starts with `Takes`/`Option` sentences (before every other
sentence except `Note`/`About`) is a command-line tool:

```lykaon
Takes file name as text.
Option verbose is whole number.
Option log level is text defaulting to debug.
```

- positional arguments bind to `Takes` in order; `--name value` fills an
  `Option`; a bare `--flag` is the word `on` for text options
- `--help` first prints usage and exits 0; a missing `Takes` argument or bad
  usage prints usage and exits 2
- `Read input.` reads one line from standard input;
  `For each line in input.` streams standard input line by line

## Logging

```lykaon
Use log level debug.
Log info server started.
Log warning hot.
```

- Levels `debug`, `info`, `warning`, `error`; default `info`; logs go to
  standard error, program output to standard output
- `settings.lykaon` next to the program is read into top-level names
  automatically (`Use settings.` also loads a settings module)

## Network: Fetch, Serve, WebSocket

Addresses in Lykaon code are written with words: `127 dot 0 dot 0 dot 1`,
`example dot com`. From the network tests:

```lykaon
Reply is Fetch plain 127 dot 0 dot 0 dot 1 on port 8080 home.
Say status of reply.
Say body of reply.
Posted is Post hello to plain 127 dot 0 dot 0 dot 1 on port 8080 home.
```

- `Fetch` gives a response record: `status`, `body`, `headers`; a 400+ status
  is a response, not a failure
- `Post x to <address> <page>.` posts data
- `Fetch plain example dot com without checking certificates.` skips TLS
  checking (only inside `Be careful.`)

Server (from the acceptance tests):

```lykaon
Serve on port 8080.
    When someone gets home.
        Respond with status 200 and text hello.
```

- Pages: `When someone gets <page> page.` (also `posts`); inside the block
  the request words `method`, `page`, `text`, `data`, `header`, `query` and
  `parameter <name>` are available
- Answers: `Respond with <text>.`, `Respond with status 200 and text <text>.`,
  `Respond with json x.`, `Respond with status N.`
- `Serve securely on port 8443 using certificate file C and key file K.` — TLS
- WebSocket: `When someone opens <page> page.`; inside, `connection` is a
  value: `Send welcome to connection.` and `Receive from connection.`
  (maybe text, missing when the other side closes)
- Sessions: `Put hello in session under name.` inside a handler