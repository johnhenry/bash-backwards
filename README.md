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

## Also Nice: Reusing Values Without Variables

Bash forces temp variables the moment you need a result twice:

```bash
# bash
count=$(find . -name "*.log" | wc -l)
echo "Found $count log files"
[ "$count" -gt 10 ] && rotate-logs
```

```bash
# hsab — dup keeps the value for multiple consumers
*.log find wc -l dup dup
"Found " swap " files" suffix suffix echo
10 gt? [rotate-logs] [] if
```

Small win in a script, but in interactive use, never naming throwaway variables is a real ergonomic gain.

---

## Also Nice: Operating on Pairs

Stack manipulation replaces named variables for "I need original AND derived":

```bash
# Rename with transformation — no variables needed
> -1 ls spread [
    dup                        # keep original
    "." split1 drop            # extract stem
    "_backup" suffix           # new name
    swap mv                    # mv original new
  ] each
```

```bash
# bash equivalent
for f in *; do
  stem="${f%.*}"
  mv "$f" "${stem}_backup"
done
```

Similar length, but hsab has no `stem=` assignment. Need the extension too? Add another `dup` and `split1`. In bash, that's another variable.

---

## Also Nice: Blocks as Values

Blocks `[...]` are first-class, so you can pass behavior around:

```bash
# Define a retry wrapper
[swap 0 [over @ 0 =? [drop drop drop true] [drop 1 plus 2dup le? [] [drop drop false] if 1 sleep] if] while] :retry

# Use it
3 [curl -s https://api.example.com/health] retry
```

The command block is a stack value — you can `dup` it, pass it to multiple consumers, compose it. Bash's `"$@"` is a one-shot trick that doesn't compose.

---

## Stack Shortcuts

| Shortcut | Action |
|----------|--------|
| **Alt+↑** | Push first word → stack |
| **Alt+↓** | Pop one → input |
| **Alt+A** | Push ALL words → stack |
| **Alt+a** | Pop ALL → input |
| **Alt+k** | Clear stack |
| `.s` | Show stack |
| `.use` / `.use=N` | Move stack items to input |

**Terminal setup (macOS):**
- iTerm2: Preferences → Profiles → Keys → "Option key acts as: Esc+"
- Terminal.app: Preferences → Profiles → Keyboard → "Use Option as Meta key"

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
- REPL-style data manipulation

**Use bash instead for:**
- Linear pipelines (`cat | grep | sort | uniq`)
- Simple conditionals and loops in scripts
- Portability — bash is everywhere
- One-liners where muscle memory wins
- String manipulation (`${var//pattern/replace}` is hard to beat)

---

## Command Line

```
hsab                    Interactive REPL
hsab -c <command>       Execute command
hsab <script.hsab>      Run script file
hsab init               Install standard library
hsab --trace            Show stack after each operation
```

---

## License

MIT
