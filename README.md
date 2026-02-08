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
| `keep` | Filter: keep if predicate passes | `spread [test -d] keep` |
| `collect` | Gather items into one value | `spread ... collect` |

```bash
# Process each file (like xargs)
-1 ls spread [.bak reext] each    # Add .bak to each filename

# Filter to directories only
-1 ls spread [test -d] keep collect

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
[test -f config.txt] [loaded echo] [missing echo] if

# Note: condition uses exit code (0 = true)
[true] [yes echo] [no echo] if   # prints: yes
[false] [yes echo] [no echo] if  # prints: no
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
| `<(cmd)` | **Workaround** | Use `cmd [consumer] \|` or `#!bash` |
| `>(cmd)` | **Use #!bash** | Process substitution needs bash |
| `` `cmd` `` | **Works** | Legacy command substitution, passes to bash |
| `{a,b,c}` | **Works** | Brace expansion happens in bash |

### Process Substitution

Bash's `<(cmd)` creates a virtual file from command output. In hsab:

```bash
# Bash
cat <(pwd)

# hsab equivalents
pwd [cat] |              # Pipe (preferred)
#!bash cat <(pwd)        # Bash passthrough

# For complex cases like diff <(cmd1) <(cmd2)
#!bash diff <(ls /a) <(ls /b)
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
[test -f config.txt] [loaded echo] &&
[test -f config.txt] [missing echo] ||
```

## Command Line Options

```
hsab                    Start interactive REPL
hsab -c <command>       Execute a single command
hsab <script.hsab>      Execute a script file
hsab --help             Show help message
hsab --version          Show version
```

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
