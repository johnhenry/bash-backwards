# hsab — A Shell for Interactive Exploration

**hsab** is a stack-based shell where results persist between commands.

Unlike bash, where each command starts fresh, hsab lets you build up, inspect, filter, and transform results incrementally. Think of it as a REPL for your filesystem and data.

```bash
# Explore incrementally — results stay on the stack
*.rs ls                    # List rust files
spread                     # Explode filenames onto stack
[wc -l] each              # Count lines in each
collect                    # Gather into list
# Now filter, sort, or pipe — without re-running anything

# The stack persists across commands
# Press Alt+↓ to pull values into your next command
```

## Quick Start

```bash
cargo build --release
cp target/release/hsab /usr/local/bin/
hsab init       # Install stdlib
hsab            # Start REPL
```

---

## The Core Idea

### Stack Persistence

Every value stays on the stack until you use it:

```bash
# Session transcript:
> hello
[hello]                           # Value sits on stack

> world
[hello, world]                    # Stack grows

> echo
world                             # Pops both, runs: echo world hello
hello
[]                                # Stack empty

# Compare two files without temp variables
> file1.txt cat
[contents of file1]

> file2.txt cat
[contents of file1, contents of file2]

> eq?                             # Are they equal?
```

### Interactive Stack Manipulation

| Shortcut | Action |
|----------|--------|
| **Alt+↓** | Pop from stack → insert at input start |
| **Alt+↑** | Push first word from input → stack |
| **Ctrl+,** | Clear the stack |
| `.use` | Move stack items to input (REPL command) |

```bash
# Build up a command incrementally:
> /var/log                        # Push path
[/var/log]

> access.log                      # Push filename
[/var/log, access.log]

# Press Alt+↓ twice to pull both into input:
> access.log /var/log tail        # Now execute
```

**Terminal setup:** On macOS, enable Option-as-Meta:
- iTerm2: Preferences → Profiles → Keys → "Option key acts as: Esc+"
- Terminal.app: Preferences → Profiles → Keyboard → "Use Option as Meta key"

---

## Where hsab Shines

### 1. Exploratory Workflows

When you don't know what you're looking for yet:

```bash
# Explore a codebase
*.rs ls spread                   # All rust files on stack
[wc -l] each                     # Line counts
collect                          # Gather results
# Hmm, let's filter...
[100 gt?] keep                   # Only files > 100 lines
# Actually, let's see the biggest ones
reverse 5 first                  # Top 5
```

In bash, you'd re-run from the start each time. In hsab, you build incrementally.

### 2. Composable Definitions

Reusable building blocks that chain naturally:

```bash
# Define once
[-1 ls spread [-f test] keep collect] :files
[-1 ls spread [-d test] keep collect] :dirs

# Use anywhere
files [wc -l] |                  # Count files in current dir
dirs [head -5] |                 # First 5 subdirectories

# Compose further
[files depth] :file-count        # Stack-based composition
```

### 3. Parallel Execution

A dedicated primitive, not ad-hoc `& & & wait`:

```bash
# Ping servers concurrently, collect results
[
  ["api.example.com" ping]
  ["db.example.com" ping]
  ["cache.example.com" ping]
] parallel

# Results come back as structured list
```

### 4. Structured Data

Work with records and tables, not just strings:

```bash
# Create records
"name" "alice" "age" 30 record   # {name: "alice", age: 30}
"name" get                       # "alice"

# Tables from commands
ls                               # Returns Table with columns
["type" get "file" eq?] where    # Filter rows
"name" sort-by                   # Sort

# Parse external data
curl -s "https://api.example.com/users" into-json
["active" get] where             # Filter to active
"email" get                      # Extract column
```

---

## Stack Operations

| Op | Effect | Example |
|----|--------|---------|
| `dup` | Duplicate top | `a dup` → `a a` |
| `swap` | Swap top two | `a b swap` → `b a` |
| `drop` | Remove top | `a b drop` → `a` |
| `over` | Copy second | `a b over` → `a b a` |
| `rot` | Rotate three | `a b c rot` → `b c a` |
| `depth` | Push stack size | `a b depth` → `a b 2` |
| `tap` | Run block, keep original | `5 [echo] tap` → prints 5, leaves 5 |
| `dip` | Temporarily hide top | `a b [echo] dip` → prints a, leaves a b |

---

## Blocks and Control Flow

Blocks `[...]` defer execution:

```bash
# Apply block
[hello echo] @                   # Runs: echo hello

# Pipe through block
ls [grep Cargo] |                # ls | grep Cargo

# Conditionals
Cargo.toml file?
  [found echo]
  [missing echo]
if

# Loops
3 [hello echo] times
[test -f ready] [sleep 1] until

# List operations
ls spread ["-f" test] keep       # Filter files
[du -h] each                     # Run on each
collect                          # Gather results
```

---

## Structured Data Operations

### Records

```bash
"name" "hsab" "version" "0.2" record   # Create
"name" get                              # Access field
"version" "0.3" set                     # Update (returns new)
keys                                    # Get all keys
merge                                   # Combine two records
```

### Tables

```bash
# Filter and sort
["age" get 30 gt?] where         # Filter rows
"name" sort-by                   # Sort
5 first                          # First 5 rows

# Aggregate
"dept" group-by                  # Group by column
count                            # Count rows
sum                              # Sum numeric column
```

### Serialization

```bash
# Parse (text → structured)
into-json    into-csv    into-kv    into-lines

# Format (structured → text)
to-json      to-csv      to-lines

# Auto-serialize for external tools
ls | grep "\.rs$"                # Table → TSV → grep
```

---

## Brace Expansion

```bash
{a,b,c}                          # Pushes a, b, c to stack
{1..5}                           # Pushes 1, 2, 3, 4, 5
file{1,2}.txt                    # Pushes file1.txt, file2.txt
{a,b}{1,2}                       # Pushes a1, a2, b1, b2
```

---

## Shell Features

### Definitions

```bash
[dup .bak suffix cp] :backup
myfile.txt backup                # cp myfile.txt myfile.txt.bak
```

### Job Control

```bash
[100 sleep] &                    # Background
jobs                             # List
fg                               # Foreground
# Ctrl+Z to suspend, bg to resume
```

### Predicates

```bash
file.txt file?                   # Is file?
src dir?                         # Is directory?
a b eq?                          # Equal?
5 10 lt?                         # Less than?
```

### Path Operations

```bash
/dir file.txt path-join          # /dir/file.txt
file _bak suffix                 # file_bak
"a.b.c" "." rsplit1              # "a.b", "c"
```

---

## Command Line

```
hsab                    Interactive REPL
hsab -c <command>       Execute command
hsab <script.hsab>      Run script file
hsab -l, --login        Login shell (source ~/.hsab_profile)
hsab --trace            Show stack after each operation
hsab init               Install standard library
```

---

## When to Use hsab

**Good fit:**
- Interactive exploration and ad-hoc data manipulation
- Building up complex operations incrementally
- Working with structured data (JSON, CSV, tables)
- Composing reusable shell functions

**Use bash instead for:**
- One-liner scripts where muscle memory wins
- Portability across systems
- Complex string manipulation (bash's `${var//pattern/replace}`)

---

## License

MIT
