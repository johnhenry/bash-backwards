# hsab — Stack-Based Structured Shell

**hsab** is a stack-based postfix shell that combines Forth-style data flow with structured data. Unlike traditional shells where data flows linearly through pipes, hsab lets you hold multiple values on a stack, manipulate them with stack operations, and work with structured records and tables—not just text.

## Why hsab?

| Capability | bash | Nushell | PowerShell | **hsab** |
|---|---|---|---|---|
| Structured data in pipeline | ✗ | ✓ | ✓ | ✓ |
| Non-linear data flow (stack) | ✗ | ✗ | ✗ | **✓** |
| External tool interop | ✓ | Fragile | Painful | ✓ |
| Multiple datasets in flight | ✗ | ✗ | ✗ | **✓** |
| Blocks as first-class values | ✗ | ✗ | Partial | ✓ |

**The unique value**: A shell where you can have two API responses on the stack, join them by user ID, filter the result, and pipe it to `grep`—all without naming a single variable.

## Quick Start

```bash
# Install
cargo build --release
cp target/release/hsab /usr/local/bin/

# Initialize stdlib
hsab init

# Start interactive shell
hsab

# Or run a command
hsab -c "hello echo"
```

## Core Concepts

### 1. Stack-Based Execution

Everything operates on a stack. Literals push, commands pop and push:

```bash
hello world          # Stack: [hello, world]
echo                 # Pops both, runs: echo world hello
                     # Stack: []

# Compare two files without variables
file1.txt cat        # Stack: [contents1]
file2.txt cat        # Stack: [contents1, contents2]
swap                 # Stack: [contents2, contents1]
eq?                  # Compare: are they equal?
```

### 2. Structured Data

Work with records and tables, not just strings:

```bash
# Create a record
"name" "alice" "age" 30 record
# Stack: [{name: "alice", age: 30}]

# Access fields
"name" get           # Stack: ["alice"]

# Parse JSON into structure
curl -s "https://api.example.com/users" into-json
["active" get] where  # Filter to active users
"email" get           # Extract emails as list

# Tables from commands
ls                    # Returns Table: name, type, size, modified
["type" get "file" eq?] where  # Filter to files only
"name" sort-by        # Sort by name
```

### 3. External Tool Interop

Structured data auto-serializes when piping to external tools:

```bash
# Table becomes TSV for grep (no manual conversion)
ls | grep "\.rs$"

# Explicit format when needed
ls | to-json | jq '.[] | .name'

# Parse external output explicitly
cat data.csv | into-csv | "amount" sort-by
```

### 4. Blocks as Values

Defer execution with blocks `[...]`:

```bash
# Block pushes without running
[hello echo]         # Stack: [Block]

# Apply (@) executes
[hello echo] @       # Runs: echo hello

# Pass blocks to control flow
[x -f test] [found echo] [missing echo] if

# Higher-order operations
ls ["size" get 1000 gt?] where   # Filter rows
["name" get] map                 # Transform column
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

```bash
# Duplicate for multiple uses
file.txt dup cat wc   # cat file.txt; wc file.txt

# Swap for argument order
dest src swap mv      # mv src dest
```

---

## Record Operations

```bash
# Construction
"name" "hsab" "version" "0.2" record    # From key-value pairs
"name=hsab\nversion=0.2" into-kv        # From text

# Access
"name" get           # Get field value
"version" "0.3" set  # Set field (returns new record)
"lang" del           # Remove field
"name" has?          # Check existence → Bool
keys                 # Get all keys → List
values               # Get all values → List

# Combine
merge                # Merge two records (top overwrites)
```

---

## Table Operations

```bash
# Construction
# Records with same keys become a table
marker
  "name" "alice" "age" 30 record
  "name" "bob" "age" 25 record
table

# From text
"name,age\nalice,30\nbob,25" into-csv
"[{\"name\":\"alice\"}]" into-json

# Column operations
| "name" "age" | select      # Keep only these columns
"age" drop-cols              # Remove column
"name" "username" rename-col # Rename
"senior" ["age" get 30 gte?] add-col  # Computed column

# Row operations
["age" get 30 gt?] where     # Filter rows
"age" sort-by                # Sort ascending
"age" sort-by reverse        # Sort descending
5 first                      # First 5 rows
3 last                       # Last 3 rows
0 nth                        # Get row as record
"dept" group-by              # Group → {dept: Table}
unique                       # Deduplicate
```

---

## Operators

### Pipe (|)
```bash
ls [grep Cargo] |        # ls | grep Cargo
ls [wc -l] |             # ls | wc -l
```

### Redirects
```bash
[hello echo] [out.txt] >     # Write stdout
[more echo] [out.txt] >>     # Append
[cat] [in.txt] <             # Read stdin
[cmd] [err.log] 2>           # Stderr
[cmd] [all.log] &>           # Both
```

### Logic
```bash
[test -f x] [found echo] &&  # Run if first succeeds
[test -f x] [missing echo] || # Run if first fails
```

### Background
```bash
[10 sleep] &             # Run in background
jobs                     # List jobs
%1 fg                    # Foreground job 1
```

---

## Control Flow

```bash
# Conditional
[condition] [then] [else] if
Cargo.toml file? [found echo] [missing echo] if

# Loops
3 [hello echo] times         # Repeat 3 times
[test -f x] [wait echo] while # While true
[test -f x] [wait echo] until # Until true

# Early exit
[
  check? [0 return] when     # Return early
  do-work
] :myfunc
```

---

## Definitions

```bash
# Define reusable words
[dup .bak suffix cp] :backup
myfile.txt backup        # cp myfile.txt myfile.txt.bak

# With local variables
[
  working TEMP local     # Scoped to this definition
  $TEMP do-something
] :myfunc
```

---

## Serialization Bridge

### Parsing (text → structured)

| Command | Input | Output |
|---------|-------|--------|
| `into-json` | JSON string | Record/Table/List |
| `into-csv` | CSV text | Table |
| `into-tsv` | TSV text | Table |
| `into-lines` | Text | List (by newlines) |
| `into-words` | Text | List (by whitespace) |
| `into-kv` | key=value text | Record |

### Formatting (structured → text)

| Command | Input | Output |
|---------|-------|--------|
| `to-json` | Record/Table | JSON string |
| `to-csv` | Table | CSV text |
| `to-tsv` | Table | TSV text |
| `to-lines` | List | Newline-separated |
| `to-kv` | Record | key=value text |
| `to-md` | Table | Markdown table |

```bash
# Real workflow: fetch, filter, export
curl -s "https://api.example.com/users" into-json
["role" get "admin" eq?] where
"email" get
to-lines
| xargs -I{} notify {}
```

---

## Structured Built-ins

| Command | Returns | Description |
|---------|---------|-------------|
| `ls` | Table | name, type, size, modified, permissions |
| `ps` | Table | pid, name, cpu, mem, user |
| `env` | Record | All environment variables |
| `open` | Record/Table | Auto-parse by file extension |
| `fetch` | String/Table | HTTP GET (--json for auto-parse) |

```bash
# Structured ls
ls ["type" get "dir" eq?] where  # Directories only
"size" sort-by reverse 10 first  # Top 10 by size

# Combine sources
ls ~/projects "name" get         # Project names
ls ~/archive "name" get          # Archive names
diff                             # What's in projects but not archive?
```

---

## Error Handling

Errors are structured values, not just exit codes:

```bash
# Errors have fields: kind, message, code, source, command
[bad-command] try
error? [
  "message" get echo    # Extract error message
] when

# Retry pattern
[fetch "https://flaky.api"] 3 retry
```

---

## Shell Features

### Job Control
```bash
[100 sleep] @        # Start foreground job
# Ctrl+Z             # Suspend (SIGTSTP)
jobs                 # List jobs (shows "Stopped")
bg                   # Resume in background (SIGCONT)
fg                   # Bring to foreground
```

### Tab Completion
- Commands from PATH
- User definitions
- File paths with tilde expansion

### Command Hashing
```bash
hash                 # Show cached command paths
ls hash              # Cache 'ls' path
-r hash              # Clear cache
```

### Login Shell
```bash
hsab -l              # Source ~/.hsab_profile
hsab --login         # Same thing
```

### Source Files
```bash
config.hsab source   # Execute in current context
. config.hsab        # Same (dot notation)
```

---

## Predicates

Clean alternatives to `test`:

```bash
# File predicates
Cargo.toml file?     # Is file?
src dir?             # Is directory?
path exists?         # Exists?

# String predicates
"" empty?            # Empty string?
a b eq?              # Strings equal?
a b ne?              # Not equal?

# Numeric predicates
5 10 lt?             # Less than?
10 5 gt?             # Greater than?
5 5 =?               # Equal?
5 10 le?             # Less or equal?
```

---

## Arithmetic

```bash
3 5 plus             # 8
10 3 minus           # 7
4 5 mul              # 20
17 5 div             # 3
17 5 mod             # 2
```

---

## String Operations

```bash
"hello" len          # 5
"hello" 1 3 slice    # "ell"
"hello" "ll" indexof # 2
/dir file.txt join   # /dir/file.txt
file _bak suffix     # file_bak
"a.b.c" "." split1   # "a", "b.c"
"a/b/c" "/" rsplit1  # "a/b", "c"
```

---

## Custom Prompts

Override `PS1`, `PS2`, `STACK_HINT` in `~/.hsabrc`:

```bash
# Colorful prompt with git status
[
  [$_GIT_BRANCH len 0 gt?]
  [" \e[33m(" $_GIT_BRANCH ")\e[0m" suffix suffix]
  [""]
  if
] :_git_info

[
  "\e[32m" $_USER "\e[0m:\e[34m"
  $_CWD "/" rsplit1 swap drop
  "\e[0m" suffix suffix suffix
  _git_info suffix " £ " suffix
] :PS1
```

**Context variables:** `$_VERSION`, `$_DEPTH`, `$_EXIT`, `$_CWD`, `$_USER`, `$_HOST`, `$_TIME`, `$_GIT_BRANCH`, `$_GIT_DIRTY`

**Colors:** `\e[32m` (green), `\e[33m` (yellow), `\e[34m` (blue), `\e[0m` (reset)

---

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Ctrl+O` | Pop from stack, insert at input start |
| `Alt+O` | Push first word from input to stack |
| `Ctrl+,` | Clear the stack |
| `Ctrl+Z` | Suspend foreground job |

---

## Command Line

```
hsab                    Start interactive shell
hsab -c <command>       Execute command
hsab <script.hsab>      Run script
hsab -l, --login        Login shell (source profile)
hsab init               Install standard library
hsab --help             Show help
hsab --version          Show version
```

---

## Standard Library

Install with `hsab init`. Includes:

- **Arithmetic**: `abs`, `min`, `max`, `inc`, `dec`
- **Strings**: `contains?`, `starts?`, `ends?`
- **Paths**: `dirname`, `basename`, `reext`
- **Lists**: `map`, `filter`, `dirs`, `files`
- **Control**: `when`, `unless`
- **Stack**: `nip`, `tuck`, `-rot`, `2drop`, `2dup`
- **Git**: `gs`, `gd`, `gl`, `ga`, `gcm`

---

## Design Philosophy

1. **Stack semantics**: Data flows through a stack, not just pipes
2. **Structured data**: Records and tables, not just text
3. **External interop**: Auto-serialize out, explicit parse in
4. **Deferred execution**: Blocks are first-class values
5. **Gradual typing**: Text works everywhere; structure is opt-in

---

## Roadmap

See [IMPLEMENTATION.md](IMPLEMENTATION.md) for the detailed development plan:

- **Phase 0**: Value type system ✓
- **Phase 1**: Record operations ✓
- **Phase 2**: Table operations (in progress)
- **Phase 3**: Structured errors
- **Phase 4**: into/to-* serialization bridge
- **Phase 5**: Structured built-ins (ls, ps return Tables)
- **Phase 6**: Joins, pivots, providers
- **Phase 7**: REPL enhancements

---

## License

MIT
