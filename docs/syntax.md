# Lykaon syntax — the 5-minute tour

Lykaon code is plain English sentences. Every sentence ends with a period (`.`).
A block is the indented sentences under an opener. There are **no quoted strings**:
words in code are raw text, and the compiler refuses quote marks (`Error: Quote in code. (T0008)`).

Every example in this tour is taken from the language's conformance test
suite and runs on the unmodified compiler.

## A minimal program

```lykaon
Say Hello World.
```

Run it with the interpreter:

```
lykaon-cli run hello.lykaon
```

or build a real executable with the compiler:

```
lykaonc-cli build fast hello.lykaon hello.exe
hello.exe
```

On Windows, `hello.exe` is a standalone native program. See [`cli.md`](cli.md)
for every option.

## Variables and data kinds

A name is introduced by `Changeable` (you may re-set it), `Constant` (may not change),
or a plain definition (also may not change):

```lykaon
Changeable score is 10.
Constant limit is 5.
Total is 20.
Set score to score plus Total.
Say score.
```

Kinds are written the way you say them:

| Kind words | Holds |
|---|---|
| `whole number` | an integer (plain numbers are whole numbers) |
| `number` | a whole or a decimal |
| `decimal` | a decimal number |
| `text` | words |
| `yes or no` | true/false |
| `maybe number` | a value or `missing` |
| `list of number`, `map of text to number`, `set of text`, `pair of number and text` | collections |
| `channel of whole number` | a channel |
| `guarded`, `atomic number`, `counter` | shared values (M4) |
| `date`, `moment`, `duration`, `secret` | time/secret values (M4) |

`Ask` reads one line from the keyboard into a name; `missing` and `fallback` handle absence:

```lykaon
Ask name.
Say hello name.
Phone is missing as maybe number.
If phone is missing say no phone.
Say phone fallback 0.
```

## Control flow

One-line `If` or an indented block, `Otherwise`, chained `Otherwise if`;
`Repeat` for loops; `For each` over collections or ranges; `Stop` and `Skip`:

```lykaon
Age is 20.
If age is at least 18 and age is at most 65.
    Say working age.
Otherwise.
    Say not working age.

Repeat 3 times say hi.
Repeat 3 times.
    Say round.

Changeable count is 1.
Repeat while count is at most 3.
    Say count.
    Set count to count plus 1.

Numbers is list of 5 and 2 and 8.
For each n in numbers.
    Say n times 2.
For each i from 1 to 5.
    Say i.
Changeable ages is map of text to number.
Put 31 in ages under Maya.
For each name and age in ages.
    Say name age.
```

Comparison phrases: `is`, `is not`, `is more than`, `is less than`,
`is at least`, `is at most`, joined with `and` / `or`.
The word `it` means *the last result*; `them` means the last list.

## Functions and modules

`To` defines a function, `Give back` returns a value, and a one-line body is allowed:

```lykaon
To head items give item 1 of items.
Numbers is list of 5 and 2 and 8.
Say head Numbers.
```

A block body with a changeable local:

```lykaon
To sumto n.
    Changeable total is 0.
    For each i from 1 to n.
        Set total to total plus i.
    Give back total.
Say sumto 100.
```

Functions can fail, and `Try` catches the failure (`it` holds the value,
`If that fails.` catches):

```lykaon
To halve n can fail.
    If n is 0 fail with zero cannot be halved.
    Give back n divided by 2.
Try halve 8.
    Say it.
If that fails.
    Say could not halve.
```

Split a program across files with `Use`, and expose names with `Share`:

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

Manage dependencies with `lykaon add`, `lykaon remove`, `lykaon update`
(see [`cli.md`](cli.md)).

## Records, enums and `Look at`

```lykaon
A person has name as text and age as number.
Maya is a person with name Maya and age 31.
Say name of maya.
```

An enum's `Look at` must cover every variant:

```lykaon
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

## Interfaces

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

## A working example: a tiny HTTP server

```lykaon
Serve on port 8080.
    When someone gets home.
        Respond with status 200 and text hello.
```

`Serve` listens until the program ends. Each request runs as its own task.
Put the program in a file and run:

```
lykaon-cli run server.lykaon
```

then open `http://localhost:8080` in a browser. Responses can also be
`Respond with hello there.` (plain text), `Respond with json x.` or
`Respond with status 404.`

## Next steps

- [`standard-library.md`](standard-library.md) — every built-in word, grouped by task
- [`cli.md`](cli.md) — run, repl, build, test, packages, benchmarks
- [`Lykaon_Language_Reference.docx`](Lykaon_Language_Reference.docx) — the complete reference for v0.1