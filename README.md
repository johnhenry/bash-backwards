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

**Guides:** [Extending stdlib](docs/extending-stdlib.md) | [Customizing prompts](docs/customizing-prompts.md)

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
| **Alt+c** | Copy top of stack to system clipboard |
| **Alt+x** | Cut top of stack to system clipboard |

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

### Local Variables

Inside definitions, `local` creates scoped variables that are restored when the function exits:

```bash
# Primitive values use env vars (shell compatible)
[myvalue _NAME local $_NAME echo] :greet

# Structured data (Lists, Tables, Maps) preserves type
[
  '[1,2,3,4,5]' into-json _NUMS local
  $_NUMS sum                   # Works! List preserved, not stringified
] :sum_list
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

## Dot-Command Convention

hsab uses a dot-prefix convention for **meta commands** — operations that affect shell state rather than data:

```bash
# Meta commands (dot-only): affect shell state
.export VAR=value       # Set environment variable
.jobs                   # List background jobs
.alias "ll" "-la ls"    # Define alias
.source script.hsab     # Source file
.copy                   # Copy to clipboard

# Shell builtins (both forms): POSIX compatibility
cd /tmp                 # or: /tmp .cd
echo hello              # or: hello .echo
test -f file            # or: file -f .test
```

**Meta commands** (dot-only): `.export`, `.unset`, `.env`, `.jobs`, `.fg`, `.bg`, `.exit`, `.tty`, `.source`, `.hash`, `.type`, `.which`, `.alias`, `.unalias`, `.trap`, `.copy`, `.cut`, `.paste`, `.plugin-*`

**Shell builtins** (both forms): `cd`, `pwd`, `echo`, `test`, `true`, `false`, `read`, `printf`, `wait`, `kill`, `pushd`, `popd`, `dirs`, `local`, `return`

---

## Full Feature Reference

Run `hsab --help` for the complete builtin reference, including:

- **Structured Data**: Records, tables, JSON parsing (`record`, `table`, `json`, `get`, `set`, `where`, `sort-by`, `select`)
- **Serialization**: Convert between text and structured data (`into-csv`, `into-tsv`, `into-json`, `to-csv`, `to-tsv`, `to-delimited`, `to-kv`)
- **File I/O**: Auto-formatting read/write (`open`, `save` — format based on extension)
- **Vector Ops**: For AI embeddings (`dot-product`, `magnitude`, `normalize`, `cosine-similarity`, `euclidean-distance`)
- **Aggregations**: Reduce lists (`sum`, `avg`, `min`, `max`, `count`, `reduce`)
- **Combinators**: Compose operations (`fanout`, `zip`, `cross`, `retry`, `compose`)
- **Filtering**: Keep/reject by predicate (`keep`, `reject`, `where`, `reject-where`, `unique`, `duplicates`)
- **Path Ops**: Manipulate paths (`path-join`, `dirname`, `basename`, `suffix`, `reext`)
- **Predicates**: File tests and comparisons (`file?`, `dir?`, `exists?`, `eq?`, `lt?`, `gt?`)
- **String Ops**: Split, slice, replace (`split1`, `rsplit1`, `len`, `slice`, `str-replace`)
- **Arithmetic**: Stack-based math (`plus`, `minus`, `mul`, `div`, `mod`)
- **Error Handling**: Try/catch for commands (`try`, `error?`, `throw`)
- **Module System**: Import and namespace support (`.import`, `namespace::func`)
- **Plugin System**: WASM plugins with hot reload (`.plugin-load`, `.plugins`, `~/.hsab/plugins/`)
- **Debugger**: Step through expressions with breakpoints (`.debug`, `.break`)
- **Media**: Terminal graphics for iTerm2/Kitty (`image-load`, `image-show`, `image-info`)
- **Links**: Clickable hyperlinks via OSC 8 (`link`, `link-info`)
- **Clipboard**: System clipboard via OSC 52 (`.copy`, `.cut`, `.paste`, `paste-here`)

### Terminal Graphics (Media Type)

hsab has a native `Media` type for images that auto-displays in terminals supporting inline graphics:

```bash
# Load an image — auto-displays in iTerm2/Kitty
"screenshot.png" image-load

# Get image metadata as a record
"photo.jpg" image-load image-info
# → {mime_type: "image/jpeg", size: 45231, width: 1920, height: 1080}

# Pipeline: load, inspect, display
"chart.png" image-load dup image-info swap image-show

# Base64 encoding for data URIs or APIs
"icon.png" image-load to-base64
```

**Supported protocols** (auto-detected):
- iTerm2 inline images (OSC 1337)
- Kitty graphics protocol (APC sequences)
- Fallback: text placeholder `[Image: image/png 800x600 5.2 KB]`

**Builtins:**
| Builtin | Stack Effect | Description |
|---------|--------------|-------------|
| `image-load` | path → Media | Load image file |
| `image-show` | Media → Media | Display image (non-destructive) |
| `image-info` | Media → Record | Get metadata |
| `to-base64` | Media → string | Encode to base64 |
| `from-base64` | mime b64 → Media | Decode from base64 |

For one-shot display, use external tools like `imgcat` (iTerm2).

### Hyperlinks (Link Type)

hsab has a native `Link` type for clickable hyperlinks in terminals supporting OSC 8:

```bash
# Create a simple link
"https://example.com" link

# Create a link with custom display text
"Click here" "https://example.com" link

# Inspect link properties
"https://example.com" link link-info
# → {url: "https://example.com"}
```

**Supported terminals**: iTerm2, Kitty, most modern terminal emulators

**Builtins:**
| Builtin | Stack Effect | Description |
|---------|--------------|-------------|
| `link` | url → Link | Create link from URL |
| `link` | text url → Link | Create link with display text |
| `link-info` | Link → Record | Get link metadata |

### Clipboard (OSC 52)

Copy and paste values using the system clipboard via OSC 52:

```bash
# Copy text to clipboard (non-destructive)
"Hello, world!" .copy
# → value stays on stack, text copied to clipboard

# Cut to clipboard (removes from stack)
"secret" .cut

# Paste from clipboard onto stack
.paste

# paste-here: expands to clipboard contents (like $VAR)
paste-here echo           # echoes clipboard contents
paste-here .bak suffix    # clipboard value + .bak extension
```

**Note**: OSC 52 clipboard support varies by terminal. Works in: iTerm2, Kitty, tmux (with `set-clipboard on`), and most modern terminal emulators.

**Builtins:**
| Builtin | Stack Effect | Description |
|---------|--------------|-------------|
| `.copy` | value → value | Copy to clipboard (non-destructive) |
| `.cut` | value → | Copy to clipboard and drop from stack |
| `.paste` | → value | Paste from clipboard onto stack |
| `paste-here` | → value | Literal that expands to clipboard contents |

### Vector Operations for AI/Embeddings

hsab treats vectors as lists of numbers, making it easy to work with embeddings from any AI API:

```bash
# Get embeddings from Ollama (or any API that returns JSON)
curl -s localhost:11434/api/embeddings -d '{"model":"nomic-embed-text","prompt":"hello"}' \
  | hsab -c 'json "embedding" get :hello_vec'

# Compare semantic similarity
hello_vec goodbye_vec cosine-similarity    # Returns -1 to 1

# Vector operations
'[3,4]' json magnitude                     # 5 (L2 norm)
'[3,4]' json normalize                     # [0.6, 0.8] (unit vector)
vec1 vec2 dot-product                      # Scalar product
vec1 vec2 euclidean-distance               # Distance
```

### Custom Aggregations with Reduce

```bash
# Sum: list init [block] reduce
'[1,2,3,4,5]' json 0 [plus] reduce         # 15

# Product
'[2,3,4]' json 1 [mul] reduce              # 24

# The block receives (accumulator, item) and returns new accumulator
```

### Combinators for Composition

```bash
# fanout: Run one value through multiple operations
"hello" [len] [upper] ["!" suffix] fanout  # 5, "HELLO", "hello!"

# zip: Pair two lists element-wise
'["a","b","c"]' json '[1,2,3]' json zip    # [[a,1], [b,2], [c,3]]

# cross: Cartesian product of two lists
'["x","y"]' json '[1,2]' json cross        # [[x,1], [x,2], [y,1], [y,2]]

# retry: Retry a block N times until success
3 [curl -s "$url"] retry                   # Tries up to 3 times

# compose: Build pipelines from blocks
[len] [2 mul] [1 plus] compose :pipeline   # Create [len 2 mul 1 plus]
"hello" pipeline                           # 11

# Dynamic pipeline construction
marker [upper] [reverse] ["!" suffix] collect compose :transform
"hello" transform                          # "!OLLEH"
```

Use cases:
- **fanout**: Test same input across multiple tools, compare outputs
- **zip**: Batch rename (old names + new names), deploy to servers
- **cross**: Test matrices (all combinations of inputs × configurations)
- **retry**: Resilient network operations, rate-limited APIs
- **compose**: Build reusable pipelines, dynamic transformation chains

See also:
- [COMPARISON.md](COMPARISON.md) — Detailed comparison with bash, fish, zsh, nushell
- [MIGRATION.md](MIGRATION.md) — Guide for migrating from bash

---

## License

MIT
