# hsab — Interactive Shell with Persistent Stack

**hsab** (bash backwards) is a shell where command results stay on a stack between commands.

This solves one specific problem that bash can't: **incremental, interactive exploration**.

```bash
# bash — every refinement restarts from scratch
$ ls -1
$ ls -1 | grep -v /
$ ls -1 | grep -v / | grep '\.rs$'
$ ls -1 | grep -v / | grep '\.rs$' | xargs wc -l
```

```bash
# hsab — results persist, you refine incrementally
> -1 ls spread              # files on stack
> [-f test] keep            # filter to regular files
> .s                        # inspect what's left
> [".rs" ends?] keep        # narrow further
> [wc -l] each              # count lines in each
```

In bash, each command is a fresh start. In hsab, intermediate results live on the stack. You inspect, filter, backtrack, and refine without retyping.

## Quick Start

```bash
cargo build --release
cp target/release/hsab /usr/local/bin/
hsab init       # Install stdlib
hsab            # Start REPL
```

---

## The One Thing Bash Can't Do

### Interactive Exploration with Persistent State

This is hsab's reason to exist. Watch a real exploration session:

```bash
> *.log ls spread                    # 47 log files land on stack
> [".gz" ends?] keep                 # narrow to compressed ones
> .s                                 # inspect: 12 files
# Hmm, too many. Let's filter by date...
> drop drop drop                     # remove 3 I don't care about
> [stat -f %m] each                  # get modification times
# Actually, let's go back to the filenames
> Alt+k                              # clear stack, start over
> *.log ls spread [-mtime -1 test] keep   # files modified today
```

**There is no bash equivalent.** In bash, you'd re-run the full pipeline from `ls` every time you want to adjust. Here, the stack holds your working set. You can:

- **Inspect** with `.s` or `depth`
- **Backtrack** with `drop` or `Alt+k`
- **Refine** with `keep`, `each`, or any filter
- **Pull values into commands** with `Alt+↓`

This is closer to spreadsheet exploration or a REPL-driven data workflow than traditional shell scripting.

---

## Other Advantages

### No Quoting Catastrophes

Bash's single biggest bug source is word splitting and glob expansion on unquoted variables. The "quote everything or die" discipline (`"$var"`, `"$(cmd)"`, `"${array[@]}"`) doesn't exist in hsab — values on the stack are discrete items, not whitespace-delimited strings waiting to explode:

```bash
# bash — unquoted $file explodes if filename has spaces
for file in $(find . -name "*.txt"); do
  process "$file"  # Must quote or die
done

# hsab — each filename is one stack entry, spaces and all
*.txt find spread [process] each
```

### Uniform Syntax

Bash has six sublanguages: `$(...)` for command substitution, `$((...))` for arithmetic, `${var%pattern}` for parameter expansion, `[[ ]]` for tests, `<()` for process substitution, `{a,b,c}` for brace expansion. hsab has one model: push values, pop values, apply blocks. You learn the stack and you know the whole language.

This matters most for people who don't write bash daily and can never remember whether it's `-eq` or `==` or which brackets to use.

### Blocks Are Values, Not Strings

In bash, passing "a piece of code" around means `eval`, `"$@"`, or function names as strings — all fragile, all quoting nightmares. In hsab, `[curl -s $url]` is a value you can store, pass, duplicate, apply:

```bash
# hsab — blocks are first-class
[curl -s https://api.example.com/health] :healthcheck
healthcheck @                 # Run it
healthcheck dup @ swap @      # Run it twice
3 healthcheck retry           # Pass to retry wrapper
```

This makes meta-programming (retry wrappers, map/filter, conditional dispatch) structurally sound rather than held together with string escaping.

### Parallel Execution Is a Primitive

Bash's `&` + `wait` gives fire-and-forget parallelism, but collecting results is painful (temp files, file descriptors, GNU parallel). hsab's `parallel` runs blocks concurrently and pushes all outputs to the stack:

```bash
[
  [api.example.com ping]
  [db.example.com ping]
  [cache.example.com ping]
] parallel
# All three results now on stack
```

### The Stack Is a Clipboard

In an interactive session, the stack acts like an infinite clipboard. Bash has `$!`, `$?`, `!!`, and `$_` as limited "memory," but no way to say "keep those three filenames around, I'll use them in a minute."

| Shortcut | Action |
|----------|--------|
| **Alt+↑** | Push first word → stack |
| **Alt+↓** | Pop one → input |
| **Alt+A** | Push ALL words → stack |
| **Alt+a** | Pop ALL → input |
| **Alt+k** | Clear stack |

### Cleaner Definitions

```bash
# hsab
[-la ls] :ll

# bash
ll() { ls -la "$@"; }
```

Minor, but hsab definitions compose — they're blocks bound to names, participating in the stack like everything else. Bash functions are mini-scripts with their own positional parameter namespace.

---

## The Tax You Pay

**Postfix notation is not an advantage.** It's a tradeoff.

Stack languages (Forth, PostScript, Factor) all face the same adoption barrier: most humans think in infix/SVO order. `file.txt cat` instead of `cat file.txt` requires retraining your instincts. This is a real cost.

You pay this tax in exchange for:
- Composability without parentheses or pipes
- No variable naming for intermediate results
- Uniform left-to-right evaluation

Whether that's worth it depends on how much time you spend in exploratory shell sessions vs. writing one-off commands.

---

## Core Operations

### Stack

| Op | Effect |
|----|--------|
| `dup` | `a` → `a a` |
| `swap` | `a b` → `b a` |
| `drop` | `a b` → `a` |
| `over` | `a b` → `a b a` |
| `rot` | `a b c` → `b c a` |
| `depth` | Push stack size |

### Blocks and Control Flow

```bash
[hello echo] @               # Apply block
ls [grep Cargo] |            # Pipe through block
file? [yes echo] [no echo] if   # Conditional
3 [hello echo] times         # Loop N times
```

### List Operations

```bash
ls spread                    # Explode to stack (with marker)
[-f test] keep               # Filter
[wc -l] each                 # Transform each
collect                      # Gather back into list
```

### Path Operations

```bash
/dir file.txt path-join      # /dir/file.txt
file .bak suffix             # file.bak
file.txt .md reext           # file.md
"a.b.c" "." rsplit1          # "a.b" "c"
```

---

## Real-World Workflows

### File Renaming with Preview

```bash
> *.txt ls spread                    # All txt files
> [dup .md reext] each               # Generate pairs: old new old new...
> .s                                 # Preview what we'll do
> [mv] each                          # Execute
```

### Git: Stage Incrementally

```bash
> --short status git spread          # Changes on stack
> ["??" starts?] keep drop           # Remove untracked indicator
> [" " split1 swap drop] each        # Extract filenames
> .s                                 # Review
> [add git] each                     # Stage selected files
```

### Log Analysis

```bash
> /var/log/app.log ERROR grep spread
> [" ERROR " rsplit1 swap drop] each  # Extract messages
> unique depth                        # Count unique errors
```

---

## When to Use hsab

**hsab is good for:**
- Interactive exploration where you don't know what you want yet
- Building up operations incrementally with inspection
- Reusing intermediate results without temp variables
- Tasks where quoting complexity would bite you
- Parallel execution with result collection

**Use bash instead for:**
- Linear pipelines (`cat | grep | sort | uniq`) — infix is more natural
- Simple scripts where portability matters — bash is everywhere
- One-liners where muscle memory wins
- Complex string manipulation (`${var//pattern/replace}` is hard to beat)
- Anything where you already know exactly what you want to do

---

## Command Line

```
hsab                    Interactive REPL
hsab -c <command>       Execute command
hsab <script.hsab>      Run script file
hsab init               Install standard library
hsab --trace            Show stack after each operation
```

**Terminal setup (macOS):**
- iTerm2: Preferences → Profiles → Keys → "Option key acts as: Esc+"
- Terminal.app: Preferences → Profiles → Keyboard → "Use Option as Meta key"

---

## Full Feature Reference

Run `hsab --help` for the complete builtin reference, including:

- **Structured Data**: Records, tables, JSON parsing (`record`, `table`, `json`, `get`, `set`, `where`, `sort-by`, `select`)
- **Serialization**: Convert between text and structured data (`into-csv`, `into-json`, `to-csv`, `to-kv`)
- **Predicates**: File tests and comparisons (`file?`, `dir?`, `exists?`, `eq?`, `lt?`, `gt?`)
- **String Ops**: Split, slice, replace (`split1`, `rsplit1`, `len`, `slice`, `str-replace`)
- **Arithmetic**: Stack-based math (`plus`, `minus`, `mul`, `div`, `mod`)
- **Error Handling**: Try/catch for commands (`try`, `error?`, `throw`)
- **Module System**: Import and namespace support (`.import`, `namespace::func`)
- **Plugin System**: WASM plugins with hot reload (`plugin-load`, `~/.hsab/plugins/`)
- **Debugger**: Step through expressions with breakpoints (`.debug`, `.break`)

See also:
- [COMPARISON.md](COMPARISON.md) — Detailed comparison with bash, fish, zsh, nushell
- [MIGRATION.md](MIGRATION.md) — Guide for migrating from bash

---

## License

MIT
