# Shell Comparison: hsab vs bash, fish, zsh, nushell

This document compares hsab with other popular shells, highlighting similarities, differences, and unique features.

## Overview

| Feature | bash | fish | zsh | nushell | hsab |
|---------|------|------|-----|---------|------|
| **Paradigm** | Traditional | Traditional | Traditional | Structured data | Stack-based postfix |
| **Syntax** | POSIX-like | Unique, clean | POSIX + extensions | Unique, pipeline | Forth-like postfix |
| **Data Types** | Strings | Strings | Strings | Tables/Records | Values (string, number, table, record) |
| **Pipes** | Text streams | Text streams | Text streams | Structured data | Stack-based with blocks |
| **Config** | .bashrc | config.fish | .zshrc | config.nu | .hsabrc |
| **Completion** | Programmable | Built-in | Programmable | Built-in | Built-in with hints |
| **Scripting** | Full | Full | Full | Full | Full |

---

## Syntax Comparison

### Simple Command

```bash
# bash/zsh
echo hello world

# fish
echo hello world

# nushell
echo hello world
# or: "hello world"

# hsab (postfix - args first, command last)
world hello echo
```

### Variable Assignment

```bash
# bash/zsh
NAME="Alice"
echo $NAME

# fish
set NAME "Alice"
echo $NAME

# nushell
let name = "Alice"
echo $name

# hsab (uses environment variables)
Alice NAME export
$NAME echo
```

### Pipes

```bash
# bash/zsh/fish
ls | grep txt | wc -l

# nushell
ls | where name =~ txt | length

# hsab (blocks for pipeline stages)
ls [grep txt] | [wc -l] |
```

### Conditionals

```bash
# bash
if [ -f file ]; then
    echo "exists"
else
    echo "not found"
fi

# fish
if test -f file
    echo "exists"
else
    echo "not found"
end

# zsh
if [[ -f file ]]; then
    echo "exists"
else
    echo "not found"
fi

# nushell
if ($file | path exists) {
    echo "exists"
} else {
    echo "not found"
}

# hsab (everything is an expression)
[file file?] [exists echo] [not\ found echo] if
```

### Loops

```bash
# bash
for i in 1 2 3; do
    echo $i
done

# fish
for i in 1 2 3
    echo $i
end

# nushell
for i in [1 2 3] {
    echo $i
}

# hsab
3 [echo] times
# or with list:
marker 1 2 3 collect [echo] each
```

### Functions

```bash
# bash
greet() {
    echo "Hello, $1"
}

# fish
function greet
    echo "Hello, $argv[1]"
end

# nushell
def greet [name: string] {
    echo $"Hello, ($name)"
}

# hsab (stack-based, args come from stack)
["Hello, " swap suffix echo] :greet
Alice greet  # => Hello, Alice
```

---

## Unique Features

### bash
- POSIX compatibility
- Ubiquitous on Unix systems
- Extensive documentation
- Process substitution: `<(cmd)`
- Here documents

### fish
- Syntax highlighting out of the box
- Web-based configuration
- Autosuggestions
- Sane scripting defaults
- Universal variables

### zsh
- Powerful globbing: `**/*.txt`
- Spelling correction
- Loadable modules
- Themeable prompts (oh-my-zsh)
- Right-side prompt

### nushell
- Structured data (tables, records)
- Built-in data manipulation
- Type system
- Errors with context
- Plugin system

### hsab
- **Stack-based postfix**: Eliminates need for `$()` command substitution
- **Visual stack hint**: See stack contents as you type
- **Block deferral**: `[cmd]` captures without executing
- **Operators as first-class**: `dup`, `swap`, `over`, `rot`
- **Step debugger**: Built-in debugging with breakpoints
- **Plugin system**: WASM-based extensibility
- **Structured data**: Tables and records (like nushell)
- **Interactive discovery**: Stack shows data flow in real-time
- **Vector operations**: Built-in support for AI/ML embeddings (`cosine-similarity`, `dot-product`, `magnitude`, `normalize`, `euclidean-distance`)
- **Reduce/fold**: Custom aggregations with `reduce` (e.g., `list 0 [plus] reduce`)

---

## Data Handling Comparison

### Traditional shells (bash, fish, zsh)
```bash
# Everything is text, parsing required
ls -la | awk '{print $9, $5}'
```

### nushell
```nu
# Structured data from the start
ls | select name size
```

### hsab
```hsab
# Structured with stack-based manipulation
ls [select name size] |
# Or using built-in table ops:
ls json unjson [name size] select
```

---

## Command Substitution

### bash/zsh
```bash
# $() captures output
echo "Files: $(ls | wc -l)"
```

### fish
```fish
# () captures output
echo "Files: "(ls | wc -l)
```

### nushell
```nu
# Interpolation
let count = (ls | length)
echo $"Files: ($count)"
```

### hsab
```hsab
# No substitution needed - stack holds values
ls [wc -l] | ["Files: " swap suffix echo] |
# Or with variable:
ls [wc -l] | COUNT export
"Files: $COUNT" echo
```

---

## Error Handling

### bash
```bash
set -e  # Exit on error
command || { echo "failed"; exit 1; }
```

### fish
```fish
command; or begin
    echo "failed"
    exit 1
end
```

### nushell
```nu
try {
    command
} catch {
    echo "failed"
}
```

### hsab
```hsab
[command] try
error? [failed echo; 1 exit] [] if
# Or: error type is on stack after try
[command] try dup error? [drop failed echo] [] if
```

---

## Configuration Files

| Shell | Config Location | Purpose |
|-------|-----------------|---------|
| bash | ~/.bashrc | Interactive non-login |
| bash | ~/.bash_profile | Login shell |
| fish | ~/.config/fish/config.fish | All sessions |
| zsh | ~/.zshrc | Interactive |
| nushell | ~/.config/nushell/config.nu | All sessions |
| hsab | ~/.hsabrc | REPL startup |
| hsab | ~/.hsab_profile | Login shell (-l) |
| hsab | ~/.hsab/lib/stdlib.hsabrc | Auto-loaded library |

---

## When to Use Each Shell

### Use **bash** when:
- Maximum portability required
- Writing system scripts
- Working with legacy systems
- Need POSIX compliance

### Use **fish** when:
- Want great defaults out of the box
- Interactive use is primary
- Teaching shell basics
- Want modern features without configuration

### Use **zsh** when:
- Want bash compatibility with extras
- Heavy customization (oh-my-zsh)
- Advanced globbing needed
- Coming from bash

### Use **nushell** when:
- Working with structured data (JSON, CSV)
- Want type safety
- Building data pipelines
- Modern language features

### Use **hsab** when:
- Want explicit data flow via stack
- Debugging command pipelines
- Learning concatenative programming
- Want to "see" your data as you type
- Building compositional workflows
- Extending with WASM plugins
- Working with AI embeddings (semantic search, similarity)
- Need built-in vector math operations

---

## Migration Paths

### From bash to hsab
See [MIGRATION.md](MIGRATION.md) for a detailed guide.

Key changes:
1. Reverse argument order (postfix)
2. Use blocks `[...]` for deferred execution
3. Pipes use block syntax: `cmd1 [cmd2] |`
4. Stack replaces command substitution

### From nushell to hsab
Similar concepts:
- Both have structured data (tables, records)
- Both support pipelines
- Both have modern error handling

Key differences:
- hsab uses postfix syntax
- hsab has explicit stack
- Pipes in hsab use blocks
- Different built-in commands

---

## Feature Matrix

| Feature | bash | fish | zsh | nushell | hsab |
|---------|:----:|:----:|:---:|:-------:|:----:|
| POSIX compatible | Yes | No | Mostly | No | No |
| Structured data | No | No | No | Yes | Yes |
| Type system | No | No | No | Yes | Partial |
| Syntax highlighting | Plugin | Yes | Plugin | Yes | Yes |
| Autosuggestions | Plugin | Yes | Plugin | Yes | Yes |
| Web config | No | Yes | No | No | No |
| Visual stack hint | No | No | No | No | Yes |
| Step debugger | No | No | No | No | Yes |
| Plugin system | No | No | No | Yes | Yes (WASM) |
| Tables built-in | No | No | No | Yes | Yes |
| Vector/embedding ops | No | No | No | No | Yes |
| Command substitution | $() | () | $() | () | Stack |

---

## Performance Notes

All shells are fast enough for interactive use. For scripting:

- **bash**: Fastest for simple scripts, well-optimized
- **fish**: Good performance, slightly slower parsing
- **zsh**: Similar to bash, plugins can slow startup
- **nushell**: Compiled, fast for data processing
- **hsab**: Rust-based, fast, stack ops are O(1)

---

## Learning Curve

| Shell | Learning Curve | Why |
|-------|---------------|-----|
| bash | Medium | Ubiquitous, but many gotchas |
| fish | Easy | Clean syntax, good errors |
| zsh | Medium | Similar to bash + extras |
| nushell | Medium | New paradigm, good docs |
| hsab | Medium-High | Postfix is different, but consistent |

For hsab, the postfix/stack paradigm is the main learning curve. Once understood, the consistency makes it predictable. The visual stack hint helps significantly with learning.

---

## Stack Advantage: Multi-Table Operations

This is where hsab's design genuinely differentiates from nushell. Nushell's pipeline is linear — one value flows left to right. hsab's stack enables non-linear data flow patterns.

### Operating on Multiple Datasets Simultaneously

```nushell
# Nushell: need variables for multiple datasets
let users = (open users.csv)
let orders = (open orders.csv)
# then join, compare, etc. — awkward
```

```hsab
# hsab: both tables on stack, no variables needed
"users.csv" open
"orders.csv" open
# stack: [users_table, orders_table]
# now operate on both directly
```

### Accumulating Results from Multiple Sources

```hsab
# Build a report from multiple queries — no intermediate variables
"users.csv" open ["status" get "active" eq?] filter count
"orders.csv" open "total" get sum
"products.csv" open count
# stack: [active_users, revenue, product_count]
"Active: %d | Revenue: $%d | Products: %d\n" printf
```

In nushell, each query would need a separate variable binding.

### Comparing API Responses

```hsab
# Fetch two API versions, diff them directly
"https://api.example.com/v1/users" fetch json
"https://api.example.com/v2/users" fetch json
# stack: [v1_table, v2_table]
"id" diff  # compare tables by ID column
```

### The Pattern

The pattern that keeps showing up: **push two or three datasets, operate on them independently, combine at the end.** This is the thing nushell's linear pipeline can't do cleanly.

### Graceful Degradation to Text

Structured data auto-serializes when piped to external commands:

```hsab
# Tables → TSV (tab-separated with header)
ls-table [grep "test"] |
# name  type  size  modified
# test.txt  file  123  1707753600

# Lists → newline-separated
'["a","b","c"]' json [cat] |
# a
# b
# c

# Flat records → key=value format
"name" "alice" "age" "30" record [cat] |
# age=30
# name=alice

# Nested records → JSON (for complex structures)
"config" "port" "8080" record record [cat] |
# {"config":{"port":"8080"}}
```

This keeps Unix tool interop working — a common criticism of nushell.

**Explicit control with `to-*` functions:**

```hsab
ls-table to-csv      # CSV format
ls-table to-json     # JSON format (array of objects)
ls-table to-lines    # newline-separated values
record to-kv         # key=value format (for records)
```

**The `into-*` family parses text into structures:**

```hsab
"key=value" into-kv          # Parse key=value → record
"name,age\na,1" into-csv     # Parse CSV → table
'{"a":1}' into-json          # Parse JSON → record/list
```

---

## Summary

hsab occupies a unique space in the shell ecosystem:

1. **Different paradigm**: Stack-based postfix vs. traditional prefix
2. **Visual feedback**: Stack hint shows data flow in real-time
3. **Debuggable**: Built-in step debugger for understanding execution
4. **Structured**: Tables and records like nushell
5. **Extensible**: WASM plugin system
6. **Compositional**: Small operations combine into powerful workflows

It's best suited for users who:
- Want to understand exactly how their commands work
- Appreciate functional/concatenative programming
- Need to debug complex pipelines
- Want structured data without leaving the shell
