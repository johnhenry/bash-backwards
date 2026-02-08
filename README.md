# hsab v2

**hsab** (Hash Backwards) is a stack-based postfix shell. Values push to a stack, executables pop their arguments and push their output. This creates a natural data-flow where output threads through commands automatically.

## Installation

```bash
cargo build --release
cp target/release/hsab /usr/local/bin/
```

## Quick Start

```bash
# Start the interactive REPL
hsab

# Execute a single command
hsab -c "hello echo"

# Run a script file
hsab script.hsab
```

## Core Concept: Stack-Based Execution

In hsab, everything operates on a stack:

1. **Literals** push themselves to the stack
2. **Executables** pop arguments, run, and push their output
3. **Blocks** are deferred execution units (like lambdas)

```bash
# Literals push to stack
hello world          # Stack: [hello, world]

# Command pops args, runs, pushes output
hello echo           # echo hello → Stack: [output]

# Multiple args (LIFO order)
world hello echo     # echo world hello (world pushed first)
```

## Command Substitution

When commands produce output, that output becomes arguments for the next command:

```bash
# pwd runs, pushes output, ls uses it as argument
pwd ls               # ls $(pwd)

# Chain commands naturally
pwd ls               # List current directory contents

# Empty output becomes nil (skipped)
true ls              # true produces no output, ls runs with no extra args
```

## Blocks and Apply

Blocks `[...]` are deferred execution - they push to the stack without running:

```bash
# Block pushes without execution
[hello echo]         # Stack: [Block([hello, echo])]

# Apply (@) executes the block
[hello echo] @       # Runs: echo hello

# Args before block are available
world [echo] @       # echo world (world was on stack)
```

## Operators

### Pipe (|)

Connect command output to another command:

```bash
# Pipe output to consumer block
ls [grep Cargo] |    # ls | grep Cargo
ls [wc -l] |         # ls | wc -l
```

### Redirects (> >> <)

Redirect input/output to files:

```bash
# Write to file
[hello echo] [out.txt] >    # echo hello > out.txt

# Append to file
[more echo] [out.txt] >>    # echo more >> out.txt

# Read from file
[cat] [input.txt] <         # cat < input.txt
```

### Logic Operators (&& ||)

Conditional execution:

```bash
# AND: run second if first succeeds
[true] [done echo] &&       # true && echo done

# OR: run second if first fails
[false] [failed echo] ||    # false || echo failed
```

### Background (&)

Run in background:

```bash
[10 sleep] &                # sleep 10 &
```

## Stack Operations

Manipulate the stack directly (inspired by Forth):

| Operation | Effect | Example |
|-----------|--------|---------|
| `dup` | Duplicate top | `a b dup` → `a b b` |
| `swap` | Swap top two | `a b swap` → `b a` |
| `drop` | Remove top | `a b drop` → `a` |
| `over` | Copy second | `a b over` → `a b a` |
| `rot` | Rotate three | `a b c rot` → `b c a` |

```bash
# Duplicate an argument
myfile.txt dup cat          # cat myfile.txt myfile.txt

# Swap for commands expecting different arg order
src dest swap mv            # mv dest src → mv src dest
```

## Path Operations

Manipulate filenames and paths:

| Operation | Effect | Example |
|-----------|--------|---------|
| `join` | Join path components | `/dir file.txt join` → `/dir/file.txt` |
| `basename` | Extract filename (no ext) | `/path/file.txt basename` → `file` |
| `dirname` | Extract directory | `/path/file.txt dirname` → `/path` |
| `suffix` | Append suffix | `file _bak suffix` → `file_bak` |
| `reext` | Replace extension | `file.txt .md reext` → `file.md` |

```bash
# Create backup filename
myfile.txt .bak reext       # myfile.bak

# Join path components
/var/log access.log join    # /var/log/access.log

# Get basename
/home/user/doc.pdf basename # doc
```

## List Operations

Process multiple items using stack markers:

| Operation | Effect | Example |
|-----------|--------|---------|
| `spread` | Split by lines onto stack | `"a\nb" spread` → marker, `a`, `b` |
| `each` | Apply block to each item | `spread [echo] each` |
| `keep` | Filter: keep if predicate passes | `spread [-d test] keep` |
| `collect` | Gather items into one value | `spread ... collect` |

```bash
# Process each file (like xargs)
-1 ls spread [.bak reext] each    # Add .bak to each filename

# Filter to directories only
-1 ls spread [-d test] keep collect

# Transform and collect
-1 ls spread [basename] each collect
```

## Definitions

Define reusable words (functions) using `:name`:

```bash
# Define a word
[dup .bak reext cp] :backup

# Use it
myfile.txt backup              # cp myfile.txt myfile.bak

# Define in ~/.hsabrc for persistence
[-la ls] :ll
[.git/config cat] :gitconf
```

## Control Flow

Conditional execution with blocks:

```bash
# if: [condition] [then] [else] if
[config.txt -f test] [loaded echo] [missing echo] if

# Note: condition uses exit code (0 = true)
[true] ["yes" echo] ["no" echo] if   # prints: yes
[false] ["yes" echo] ["no" echo] if  # prints: no

# times: repeat a block N times
3 [hello echo] times               # prints: hello hello hello

# while: repeat while condition passes (exit code 0)
[/tmp/flag -f test] [waiting... echo] while

# until: repeat until condition passes
[/tmp/ready -f test] [waiting... echo] until

# Multi-file redirect (writes to all files)
[data echo] [a.txt b.txt c.txt] >  # writes "data" to all three files
```

## Parallel Execution

Run multiple commands concurrently:

```bash
# parallel: run blocks in parallel, wait for all, collect output
[[task1 echo] [task2 echo] [task3 echo]] parallel

# fork: background N blocks from stack (fire and forget)
[long-task-1] [long-task-2] 2 fork
```

## Process Substitution

Create temp files from command output (like bash's `<(cmd)`):

```bash
# subst: run command, push temp file path
[/dir1 ls] subst [/dir2 ls] subst diff  # diff <(ls /dir1) <(ls /dir2)

# Useful for commands expecting file arguments
[-la ls] subst cat                       # cat <(ls -la)
```

## Interactive Commands (TTY)

Run commands that need terminal access:

```bash
# tty: run with direct TTY access
[file.txt vim] tty                # Edit file in vim
[file.txt less] tty               # View file in less
[top] tty                         # Run top interactively
[python3] tty                     # Python REPL
```

Use `tty` for any command that:
- Needs keyboard input (editors, REPLs)
- Uses terminal features (colors, cursor movement)
- Expects to be run interactively

## Comments

```bash
hello echo # this is ignored
ls # list files
```

Comments work inline and in scripts. They're stripped before parsing, respecting quotes:
```bash
"#not a comment" echo  # this IS a comment
```

## Bash Passthrough

For complex bash that doesn't fit the postfix model:

```bash
#!bash for i in 1 2 3; do echo $i; done
#!bash echo -e 'line1\nline2'
```

## Variables

Shell variables pass through to bash:

```bash
$HOME echo              # echo $HOME
$USER echo              # echo $USER
```

## Bash Compatibility

hsab translates to bash, so many bash constructs work directly:

| Bash syntax | Status | Notes |
|-------------|--------|-------|
| `$(cmd)` | **Native** | hsab's core feature! `pwd ls` = `ls $(pwd)` |
| `${VAR}` | **Works** | Passes through to bash |
| `${VAR:-default}` | **Works** | All parameter expansion works |
| `$((1+2))` | **Works** | Arithmetic passes through |
| `<(cmd)` | **Use subst** | `[cmd] subst` creates temp file with output |
| `>(cmd)` | **Use #!bash** | Output process substitution needs bash |
| `` `cmd` `` | **Works** | Legacy command substitution, passes to bash |
| `{a,b,c}` | **Works** | Brace expansion happens in bash |

### Process Substitution

Bash's `<(cmd)` creates a virtual file from command output. In hsab, use `subst`:

```bash
# Bash: diff <(ls /a) <(ls /b)
# hsab:
[/a ls] subst [/b ls] subst diff

# For simple cases, pipe is often cleaner
pwd [cat] |              # cat <(pwd)
```

## Quoting

Quotes preserve strings and prevent executable detection:

```bash
# Double quotes
"hello world" echo      # echo "hello world"

# Single quotes (literal)
'$HOME' echo            # echo '$HOME'

# Quote command names to use as args
"ls" echo               # echo "ls" (doesn't execute ls)
```

## Built-in REPL Commands

| Command | Description |
|---------|-------------|
| `.help` / `.h` | Show help message |
| `.stack` / `.s` | Show current stack |
| `.pop` / `.p` | Pop and show top value |
| `.clear` / `.c` | Clear the stack |
| `exit` / `quit` | Exit the REPL |

## Script Files

Create `.hsab` files:

```bash
# example.hsab
# Lines starting with # are comments (except #!bash)

# Simple commands
hello echo

# Pipe chains
ls [grep txt] |

# Path operations
/path/to/file.txt basename
```

Run with:

```bash
hsab example.hsab
```

## Startup File

When starting the REPL, hsab loads `~/.hsabrc` if it exists:

```bash
# ~/.hsabrc
"Welcome to hsab!" echo
```

Set `HSAB_BANNER=1` to show the startup banner:

```bash
HSAB_BANNER=1 hsab
```

## Standard Library (Example ~/.hsabrc)

The primitives (`spread`, `each`, `keep`, `collect`, `if`) enable building higher-level constructs in hsab itself:

```bash
# ~/.hsabrc - hsab standard library

# === Aliases ===
[-la ls] :ll
[-1 ls] :l1
[.git/config cat] :gitconf

# === File operations ===
[dup .bak reext cp] :backup           # file.txt backup → cp file.txt file.bak
[dup .orig reext swap mv] :mv-orig    # file.txt mv-orig → mv file.txt file.orig

# === List operations (built on primitives) ===

# map: transform each item and collect
# Usage: items [transform] map
[each collect] :map

# filter: keep items matching predicate
# Usage: items [predicate] filter
[keep collect] :filter

# dirs: list only directories
[-1 ls spread [-d test] keep collect] :dirs

# files: list only regular files
[-1 ls spread [-f test] keep collect] :files

# exes: list only executables
[-1 ls spread [-x test] keep collect] :exes

# basenames: get basenames of all files
[-1 ls spread [basename] each collect] :basenames

# === Control flow helpers ===

# unless: opposite of if (run then-block if condition FAILS)
# Usage: [cond] [then] [else] unless
[swap if] :unless

# when: if without else (noop if fails)
# Usage: [cond] [then] when
[[] if] :when
```

### Usage Examples

```bash
# Using the standard library definitions
myfile.txt backup              # Creates myfile.bak

dirs                           # List only directories
files                          # List only regular files

# Filter with custom predicate
-1 ls spread [-s test] filter  # Non-empty files only

# Map with transform
-1 ls [.bak reext] map         # Add .bak to all filenames

# Conditional
[config -f test] [loaded echo] when
```

## Examples

### Basic Commands

```bash
hello echo              # echo hello
-la ls                  # ls -la
world hello echo        # echo world hello
```

### Command Chaining

```bash
pwd ls                  # ls $(pwd) - list current directory
ls [grep txt] |         # ls | grep txt
ls [wc -l] |            # ls | wc -l
```

### File Operations

```bash
# Create backup
myfile.txt dup .bak reext swap cp
# → cp myfile.txt myfile.bak

# Join and read
/var/log syslog join cat
# → cat /var/log/syslog
```

### Stack Manipulation

```bash
# Duplicate for two uses
data.txt dup cat [wc -l] |
# → Shows file content and line count

# Swap for correct argument order
dest src swap mv        # mv src dest
```

### Conditional Execution

```bash
[config.txt -f test] [loaded echo] &&
[config.txt -f test] [missing echo] ||
```

## Command Line Options

```
hsab                    Start interactive REPL
hsab -c <command>       Execute a single command
hsab <script.hsab>      Execute a script file
hsab --help             Show help message
hsab --version          Show version
```

## Execution Model

hsab uses a **persistent bash subprocess** for all command execution:

1. **Output Capture**: Command output is captured via markers and pushed to the stack
2. **Synchronous by Default**: Commands block until complete, output becomes stack value
3. **Background (`&`)**: Sends command to bash with `&`, returns immediately
4. **Parallel**: Runs `(cmd1) & (cmd2) & wait`, collects all output

### Limitations

- **Interactive commands need `tty`** - Commands like `vim`, `less`, `top` need TTY access
  - Use: `[file.txt vim] tty` to run interactively
- **No job control** - Background jobs can't be brought to foreground
- **Temp files from `subst`** - Not automatically cleaned up (use `/tmp`)

### Why This Design?

The stack-based model requires capturing command output to push it for the next command. This fundamental choice means we trade interactive capability for powerful command composition.

## Design Philosophy

hsab v2 is built on these principles:

1. **Stack semantics**: Data flows through a stack, commands pop and push
2. **Output threading**: Command output automatically becomes input for the next
3. **Deferred execution**: Blocks `[...]` are first-class values
4. **Explicit control**: Operators like `@` `|` `&&` make data flow visible
5. **Bash interop**: Falls back to bash for complex operations

## Tips

1. **Think in stacks**: Values push, commands pop
2. **Use blocks for grouping**: `[cmd args]` defers execution
3. **Quote command names**: `"ls" echo` to use as string
4. **Apply for execution**: `[cmd] @` runs a block
5. **Pipes for data flow**: `cmd [consumer] |`
6. **Stack ops for reordering**: `swap`, `dup`, `over`
7. **Escape to bash**: `#!bash` for complex logic

## License

MIT
