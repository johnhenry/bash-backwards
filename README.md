# hsab

**hsab** (Hash Backwards) is a postfix notation shell that transpiles to bash. Instead of writing `command args`, you write `args command`. The shell auto-detects executables and stops parsing at the first one found, putting remaining tokens back on your input for the next command.

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
hsab -c "-la ls"

# Run a script file
hsab script.hsab

# Show generated bash without executing
hsab --emit "-la ls"
```

## Core Concept: Postfix Notation

In hsab, arguments come **before** the command:

| hsab | bash |
|------|------|
| `-la ls` | `ls -la` |
| `hello echo` | `echo hello` |
| `world hello echo` | `echo world hello` |
| `status git` | `git status` |
| `file.txt cat` | `cat file.txt` |

## Executable-Aware Parsing

hsab automatically detects when a word is an executable (via a built-in list of ~120 common commands + PATH lookup). Parsing **stops** at the first executable found.

```
hsab-0.1.0£ -la ls
# Executes: ls -la

hsab-0.1.0£ ls -la
# Executes: ls
# Leftovers: -la (next prompt shows ¢)
hsab-0.1.0¢ -la
```

This means:
- **Postfix order** (`args command`): Args are consumed by the command
- **Traditional order** (`command args`): Command runs alone, args become leftovers

### Leftovers

When parsing stops, remaining tokens are "leftovers" that appear pre-filled on your next prompt. The prompt changes from `£` to `¢` to indicate leftovers are present:

```
hsab-0.1.0£ cat file.txt grep pattern
# Executes: cat
# Next prompt shows leftovers with ¢:
hsab-0.1.0¢ file.txt grep pattern
```

This enables a natural workflow where you build up commands incrementally.

## Pipes and Quotations

To create pipes, use **quotations** with `[]`:

```bash
# Single command
-la ls                        # ls -la

# Piped commands (use quotations)
[hello grep] ls               # ls | grep hello
[pattern grep] file.txt cat   # cat file.txt | grep pattern

# Multi-stage pipes
[-5 head] [txt grep] ls       # ls | grep txt | head -5
```

Inside a quotation, the syntax is still postfix: `[args command]` becomes `command args` in the pipe.

## Stack Operations

Inspired by Forth and other stack-based languages, hsab provides stack operations that manipulate the argument list during parsing:

| Operation | Effect | Example |
|-----------|--------|---------|
| `dup` | Duplicate top | `a b dup` → `a b b` |
| `swap` | Swap top two | `a b swap` → `b a` |
| `drop` | Remove top | `a b drop` → `a` |
| `over` | Copy second | `a b over` → `a b a` |
| `rot` | Rotate three | `a b c rot` → `b c a` |

### Examples

```bash
# Duplicate an argument
file.txt dup cat              # cat file.txt file.txt

# Swap order (useful for cp/mv)
dest src swap cp              # cp src dest

# Copy file to same name in different dir
file.txt /backup swap over join cp
                              # cp file.txt /backup/file.txt

# Chain operations
a b c rot drop swap echo      # echo c a
```

## Path Operations

Path operations manipulate filenames and paths:

| Operation | Effect | Example |
|-----------|--------|---------|
| `join` | Join path components | `/dir file.txt join` → `/dir/file.txt` |
| `basename` | Extract filename (no ext) | `/path/file.txt basename` → `file` |
| `dirname` | Extract directory | `/path/file.txt dirname` → `/path` |
| `suffix` | Append suffix | `file _bak suffix` → `file_bak` |
| `reext` | Replace extension | `file.txt .md reext` → `file.md` |

### Examples

```bash
# Get basename of a path
/home/user/document.pdf basename echo
                              # echo document

# Create backup filename
file.txt dup ".bak" reext cp  # cp file.txt file.bak

# Join path components
/var/log access.log join cat  # cat /var/log/access.log

# Complex: backup to different dir
file.txt dup dirname swap basename "_backup" suffix ".txt" reext swap over join cp
```

## Operators

### Logical Operators

```bash
# AND: run second command if first succeeds
ls [done echo] &&             # ls && echo done

# OR: run second command if first fails
ls [failed echo] ||           # ls || echo failed
```

### Redirects

```bash
# Write to file
hello echo [out.txt] >        # echo hello > out.txt

# Append to file
more echo [out.txt] >>        # echo more >> out.txt

# Read from file
cat [input.txt] <             # cat < input.txt
```

### Background Execution

```bash
10 sleep &                    # sleep 10 &
```

## hsab Variables

hsab tracks state between commands with special `%` variables:

| Variable | Description |
|----------|-------------|
| `%_` | Last argument of previous command |
| `%!` | Stdout of previous command (trimmed) |
| `%?` | Exit code of previous command |
| `%cmd` | The generated bash command |
| `%@` | All arguments of previous command |
| `%0`, `%1`, `%2`... | Individual lines of output (0-indexed) |

### Examples

```bash
hsab-0.1.0£ ls
Cargo.lock
Cargo.toml
README.md
src

hsab-0.1.0£ %0 cat              # cat Cargo.lock (first line of ls output)

hsab-0.1.0£ hello echo
hello

hsab-0.1.0£ %! wc -c            # wc -c hello (count chars in "hello")

hsab-0.1.0£ false
hsab-0.1.0£ %? echo             # echo 1 (exit code of false)
```

### Line Indexing Workflow

```bash
hsab-0.1.0£ ls
file1.txt
file2.txt
file3.txt

hsab-0.1.0£ %1 cat              # cat file2.txt (second file)
hsab-0.1.0£ %2 rm               # rm file3.txt (third file)
```

## Startup File

When starting the interactive REPL, hsab loads `~/.hsabrc` if it exists. This file is executed line by line using hsab syntax:

```bash
# ~/.hsabrc - runs on REPL startup

# Display a greeting
"Welcome to hsab!" echo

# Set up environment
#!bash export EDITOR=vim

# Source additional hsab files
~/.hsab/aliases.hsab source

# Any hsab command works here
```

The startup banner is hidden by default. To show it, set `HSAB_BANNER=1`:
```bash
HSAB_BANNER=1 hsab
```

## Sourcing Files

The `source` command is smart about file types:

```bash
# Source hsab files (processed by hsab)
aliases.hsab source           # Executes through hsab parser

# Source bash files (passed to bash)
setup.sh source               # Executes through bash
```

This allows you to organize hsab code into modules:

```bash
# ~/.hsab/git-helpers.hsab
# Git workflow helpers - source this in your hsabrc

# Quick commit with message
# Usage: "message" gc
#!bash function gc() { git commit -m "$1"; }
```

Variables set in sourced files persist in the session:

```bash
hsab-0.1.0£ config.hsab source   # Sets MYVAR=hello
hsab-0.1.0£ $MYVAR echo          # Prints: hello
```

## Bash Passthrough

For complex bash that doesn't fit the postfix model, use `#!bash`:

```bash
hsab-0.1.0£ #!bash for i in 1 2 3; do echo $i; done
1
2
3

hsab-0.1.0£ #!bash echo -e 'line1\nline2\nline3'
line1
line2
line3
```

## Quoting

Quotes work as expected and prevent executable detection:

```bash
# Double quotes
"hello world" echo            # echo "hello world"

# Single quotes
'$HOME' echo                  # echo '$HOME' (literal)

# Quote an executable name to use it as an argument
"ls" echo                     # echo "ls" (prints "ls", doesn't list files)
```

## Variables and Parameters

hsab has two types of variables: **bash variables** (from the shell environment) and **hsab variables** (from previous command state).

### Bash Variables

Shell variables like `$HOME`, `$USER`, `$PATH` pass through to bash unchanged:

```bash
$HOME echo                    # echo $HOME
${USER} echo                  # echo ${USER}
"$PATH" echo                  # echo "$PATH"
```

### Variable Assignment

Variable assignment works in postfix notation:

```bash
MYVAR=hello export            # export MYVAR=hello
MYVAR=world echo              # echo MYVAR=world (not assignment!)
```

**Important:** `VAR=value command` syntax (inline assignment) requires `#!bash`:

```bash
#!bash MYVAR=hello echo $MYVAR    # Prints: hello
```

### Variable Persistence

hsab uses a **persistent bash subprocess**, so variables set in one command **are available** in subsequent commands within the same session:

```bash
hsab-0.1.0£ MYVAR=hello export
hsab-0.1.0£ $MYVAR echo
hello

hsab-0.1.0£ MYVAR=world export
hsab-0.1.0£ $MYVAR echo
world
```

This works in both the REPL and script files. Each `hsab` invocation (REPL session or script run) gets its own bash subprocess.

**Note:** Separate `hsab -c` invocations don't share variables:

```bash
$ hsab -c "MYVAR=hello export"   # One subprocess
$ hsab -c '$MYVAR echo'          # Different subprocess - MYVAR not set
```

**Additional ways to pass data:**

1. **Use hsab's `%` variables** for command-to-command data:
   ```bash
   hsab-0.1.0£ hello echo
   hello
   hsab-0.1.0£ %! cat              # %! contains "hello"
   hello
   ```

2. **Set environment variables before launching hsab:**
   ```bash
   $ MYVAR=preset hsab
   hsab-0.1.0£ $MYVAR echo
   preset
   ```

### The `export` Command

`export` is recognized as an executable and works in postfix:

```bash
MYVAR=hello export            # export MYVAR=hello
PATH=/custom:$PATH export     # export PATH=/custom:$PATH
```

However, exported variables only persist within that single command's subshell. For persistent environment changes, use `#!bash` or set them before launching hsab.

### hsab Variables (`%` Variables)

Unlike bash variables, hsab's `%` variables **persist across commands** because they're managed by hsab itself (not bash). See the [hsab Variables](#hsab-variables) section above for the full list and examples.

Key difference: `%` variables are expanded **before** the command is sent to bash, while `$` variables are expanded **by** bash.

### Parameter Expansion

Bash parameter expansion syntax passes through:

```bash
${HOME} echo                  # echo ${HOME}
${VAR:-default} echo          # echo ${VAR:-default}
${#PATH} echo                 # echo ${#PATH} (length)
```

For complex parameter expansion, use `#!bash`:

```bash
#!bash echo ${PATH//:/\\n}    # Replace : with newlines
```

## Built-in REPL Commands

| Command | Description |
|---------|-------------|
| `help` | Show help message |
| `state` | Show current shell state (%vars) |
| `exit` / `quit` | Exit the REPL |

## Script Files

Create a file with `.hsab` extension:

```bash
# example.hsab
# Lines starting with # are comments (except #!bash)

# List files and filter
[Cargo grep] ls

# Use previous output
%0 cat

# Bash passthrough for complex logic
#!bash echo "Done!"
```

Run with:

```bash
hsab example.hsab
```

## Command Line Options

```
hsab                    Start interactive REPL (loads ~/.hsabrc)
hsab -c <command>       Execute a single command
hsab <script.hsab>      Execute a script file
hsab --emit <command>   Show generated bash without executing
hsab --help             Show help message
hsab --version          Show version
```

## How It Works

hsab processes input through a pipeline:

1. **Expand** `%vars` with values from previous command
2. **Tokenize** input into words, quotes, operators
3. **Parse** with executable detection (stops at first executable)
4. **Apply** stack/path operations during parsing
5. **Transform** postfix AST to infix order
6. **Emit** bash code
7. **Execute** via persistent bash subprocess
8. **Update** state for next command

## Examples

### File Operations

```bash
# List, filter, and examine
[Cargo grep] ls               # ls | grep Cargo
%0 cat                        # cat first match

# Find and process
[*.rs grep] find .            # find . | grep *.rs
%0 wc -l                      # count lines in first result

# Create backup with path ops
file.txt dup .bak reext cp    # cp file.txt file.bak
```

### Git Workflow

```bash
status git                    # git status
[. add] git                   # git add .
["fix bug" -m commit] git     # git commit -m "fix bug"
```

### Text Processing

```bash
#!bash echo -e 'apple\nbanana\ncherry'
[a grep] %!                   # grep a on previous output
%! wc -l                      # count matching lines
```

### Chained Operations

```bash
# ls, grep for .rs files, count them
[-l wc] [rs grep] ls          # ls | grep rs | wc -l

# With error handling
[error echo] [missing cat] || # cat missing || echo error
```

### Stack Operations in Practice

```bash
# Copy file to backup directory preserving name
src.txt /backup swap over basename swap join cp
# Result: cp src.txt /backup/src

# Swap arguments for commands that expect dest first
/dest /src swap mv            # mv /src /dest
```

## Design Philosophy

hsab explores an alternative shell syntax where:

1. **Data flows left-to-right**: You describe *what* you're operating on, then *how*
2. **Incremental composition**: Leftovers let you build commands piece by piece
3. **Explicit pipes**: Quotations `[]` make pipe structure visually clear
4. **State persistence**: `%` variables connect commands without subshells
5. **Stack-based manipulation**: Forth-inspired operations for argument reordering

## Tips

1. **Think backwards**: Write what you want to do to something, then what that something is
2. **Use quotations for pipes**: `[args cmd]` creates a pipe stage
3. **Leftovers are your friend**: Let partial input carry forward
4. **Use %N for selection**: After `ls`, use `%0`, `%1`, etc. to pick files
5. **Stack ops for reordering**: Use `swap`, `dup`, `over` to manipulate args
6. **Path ops for filenames**: Use `basename`, `dirname`, `reext` for path manipulation
7. **Escape to bash**: Use `#!bash` for anything too complex

## License

MIT
