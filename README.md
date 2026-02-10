# Bash Backwards (hsab)

**hsab** is a standalone stack-based postfix shell written in Rust. Values push to a stack, executables pop their arguments and push their output. This creates a natural data-flow where output threads through commands automatically.

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

## Glob and Tilde Expansion

hsab expands globs and tildes natively:

```bash
# Glob patterns
src/*.rs -l wc       # wc -l src/ast.rs src/eval.rs ...

# Tilde expansion
~/Documents ls       # ls /home/user/Documents
~ ls                 # ls /home/user
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

### Redirects (> >> < 2> 2>> &> 2>&1)

Redirect input/output to files:

```bash
# Write stdout to file
[hello echo] [out.txt] >     # echo hello > out.txt

# Append to file
[more echo] [out.txt] >>     # echo more >> out.txt

# Read from file
[cat] [input.txt] <          # cat < input.txt

# Redirect stderr
[bad-cmd] [err.log] 2>       # bad-cmd 2> err.log

# Redirect both stdout and stderr
[cmd] [all.log] &>           # cmd &> all.log

# Multi-file redirect (writes to all files)
[data echo] [a.txt b.txt c.txt] >  # writes "data" to all three files
```

### Logic Operators (&& ||)

Conditional execution:

```bash
# AND: run second if first succeeds
[true] [done echo] &&        # true && echo done

# OR: run second if first fails
[false] [failed echo] ||     # false || echo failed
```

### Background (&)

Run in background:

```bash
[10 sleep] &                 # sleep 10 &
jobs                         # List background jobs
%1 fg                        # Bring job 1 to foreground
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
| `depth` | Push stack size | `a b c depth` → `a b c 3` |

```bash
# Duplicate an argument
myfile.txt dup cat          # cat myfile.txt myfile.txt

# Swap for commands expecting different arg order
src dest swap mv            # mv dest src → mv src dest
```

## Path & String Operations

Manipulate filenames, paths, and strings:

| Operation | Effect | Example |
|-----------|--------|---------|
| `join` | Join path components | `/dir file.txt join` → `/dir/file.txt` |
| `suffix` | Append suffix | `file _bak suffix` → `file_bak` |
| `split1` | Split at first occurrence | `"a.b.c" "." split1` → `a`, `b.c` |
| `rsplit1` | Split at last occurrence | `"a/b/c" "/" rsplit1` → `a/b`, `c` |

```bash
# Join path components
/var/log access.log join    # /var/log/access.log

# Split at last slash (dirname/basename pattern)
"/path/to/file.txt" "/" rsplit1  # "/path/to", "file.txt"

# Split at first dot (stem/extension pattern)
"file.txt" "." split1            # "file", "txt"
```

See `examples/stdlib.hsabrc` for `dirname`, `basename`, `reext` definitions built on these primitives.

## List Operations

Process multiple items using stack markers:

| Operation | Effect | Example |
|-----------|--------|---------|
| `marker` | Push a marker onto stack | `marker` → boundary for each/keep/collect |
| `spread` | Split by lines onto stack | `"a\nb" spread` → marker, `a`, `b` |
| `each` | Apply block to each item | `spread [echo] each` |
| `keep` | Filter: keep if predicate passes | `spread [-d test] keep` |
| `collect` | Gather items into one value | `spread ... collect` |

```bash
# Process each file (like xargs)
-1 ls spread [.bak suffix] each   # Add .bak to each filename

# Filter to directories only
-1 ls spread [-d test] keep collect

# Custom spread with different delimiter
["a,b,c" "," split-all] :spread-csv   # (requires split-all, see stdlib)
```

## Definitions

Define reusable words (functions) using `:name`:

```bash
# Define a word
[dup .bak suffix cp] :backup

# Use it
myfile.txt backup              # cp myfile.txt myfile.txt.bak

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
[true] [yes echo] [no echo] if    # prints: yes
[false] [yes echo] [no echo] if   # prints: no

# times: repeat a block N times
3 [hello echo] times              # prints: hello hello hello

# while: repeat while condition passes (exit code 0)
[/tmp/flag -f test] [waiting... echo] while

# until: repeat until condition passes
[/tmp/ready -f test] [waiting... echo] until

# break: exit loop early
10 [dup echo dup 5 -eq test [break] [] if] times  # prints 1-5 then stops
```

## Shell Builtins

These run instantly without forking:

| Builtin | Description |
|---------|-------------|
| `cd` | Change directory (with ~ expansion) |
| `pwd` | Print working directory |
| `echo` | Print arguments |
| `printf` | Formatted output (C-style format strings) |
| `read` | Read line from stdin into variable |
| `true` | Exit with code 0 |
| `false` | Exit with code 1 |
| `test` / `[` | File and string tests |
| `export` | Set environment variable |
| `unset` | Remove environment variable |
| `env` | List all environment variables |
| `local` | Create function-local variable |
| `return` | Return from definition early |
| `jobs` | List background jobs |
| `fg` | Bring job to foreground |
| `bg` | Resume job in background |
| `wait` | Wait for background jobs to complete |
| `kill` | Send signal to process |
| `trap` | Set signal handlers |
| `pushd` | Push directory onto stack and cd |
| `popd` | Pop directory from stack and cd |
| `dirs` | Show directory stack |
| `alias` | Create command alias |
| `unalias` | Remove command alias |
| `type` | Show how command would be interpreted |
| `which` | Show command location |
| `hash` | Manage command path cache |
| `source` / `.` | Execute file in current context |
| `exit` | Exit the shell |
| `tty` | Run with inherited TTY (for vim, less, etc.) |
| `bash` | Run bash command string |

### Interactive Commands

Commands like `vim`, `less`, `top`, and REPLs work automatically - hsab detects when a command's output isn't being consumed and runs it interactively:

```bash
file.txt vim                 # Opens vim interactively
README.md less               # View with less
top                          # Runs top interactively
python3                      # Start Python REPL
```

The `tty` builtin is available as an explicit override if auto-detection doesn't work:

```bash
some-app tty                 # Force interactive mode
```

### Bash Fallback

Use `bash` for complex bash constructs that don't fit the postfix model:

```bash
# Bash for-loops
"for i in 1 2 3; do echo $i; done" bash

# Brace expansion
"echo {a,b,c}.txt" bash

# Process substitution (output form)
"diff <(ls /a) <(ls /b)" bash

# Here-strings
"cat <<< 'hello world'" bash
```

### Test Builtin (Postfix Syntax)

```bash
# File tests: path flag test
Cargo.toml -f test           # Test if file exists
src -d test                  # Test if directory
script.sh -x test            # Test if executable

# String comparison: str1 str2 op test
hello hello = test           # Strings equal (exit 0)
foo bar != test              # Strings not equal

# Numeric comparison: n1 n2 op test
5 3 -gt test                 # 5 > 3 (exit 0)
10 20 -lt test               # 10 < 20 (exit 0)
```

### Printf (Formatted Output)

C-style formatted output with escape sequences:

```bash
# Basic format specifiers
"world" "Hello, %s!\n" printf           # Hello, world!
42 "Answer: %d\n" printf                # Answer: 42
3.14159 "Pi: %f\n" printf               # Pi: 3.141590

# Multiple arguments (LIFO order - last pushed is first %s)
"world" "hello" "%s %s\n" printf        # hello world

# Escape sequences
"Line1\nLine2\tTabbed" printf           # Newlines and tabs
```

### Read (Input from Stdin)

Read a line from stdin into a variable:

```bash
# Basic read
NAME read                    # Waits for input, stores in $NAME
$NAME echo                   # Echo what was read

# Use in scripts
"Enter your name: " printf
NAME read
$NAME "Hello, %s!\n" printf
```

### Directory Stack (pushd/popd/dirs)

Navigate directories with a stack:

```bash
# Push directory and cd
/tmp pushd                   # cd to /tmp, push old dir to stack
# Output: /tmp /home/user

# Show directory stack
dirs                         # Current dir + stack

# Pop and return
popd                         # Return to previous directory

# Clear the stack
-c dirs                      # Clear directory stack
```

### Wait (Background Job Control)

Wait for background jobs to complete:

```bash
# Start background job
[5 sleep] &                  # Returns immediately
jobs                         # Shows running job

# Wait for all jobs
wait                         # Blocks until all jobs complete

# Wait for specific job
[10 sleep] &                 # Job %1
[5 sleep] &                  # Job %2
%1 wait                      # Wait only for job 1
```

### Kill (Send Signals)

Send signals to processes:

```bash
# Kill by PID (default SIGTERM)
12345 kill

# Kill with specific signal
12345 -9 kill                # SIGKILL
12345 -SIGKILL kill          # Same thing
12345 -HUP kill              # SIGHUP

# Kill background job
[100 sleep] &
%1 kill                      # Kill job 1

# Common signals: HUP(1), INT(2), QUIT(3), KILL(9), TERM(15), STOP(17), CONT(19)
```

### Trap (Signal Handlers)

Set handlers for signals using blocks (deferred execution):

```bash
# Set trap with block
[cleanup] INT trap           # On SIGINT, run the cleanup definition
[goodbye echo] EXIT trap     # On exit, echo "goodbye"

# List all traps
trap                         # Shows all traps

# Show specific trap
INT trap                     # Shows INT handler

# Clear trap (empty block)
[] INT trap                  # Clear INT handler
```

### Alias (Bash Compatibility)

Create command aliases using blocks (note: hsab definitions `:name` are more powerful):

```bash
# Create alias with block
[-la ls] ll alias            # ll expands to "ls -la"

# List all aliases
alias                        # Shows: alias ll='[-la ls]'

# Show specific alias
ll alias                     # Shows ll's expansion

# Remove alias
ll unalias

# Remove all aliases
-a unalias
```

### Local Variables in Definitions

Use `local` for function-scoped variables that restore after the definition exits:

```bash
# Define a function with local variables
[
  TEMP=working local         # TEMP is local to this function
  $TEMP echo                 # Uses local value
] :myfunc

# Original TEMP is restored after myfunc exits
working export TEMP=original
myfunc                       # Prints: working
$TEMP echo                   # Prints: original (restored!)
```

### Return (Early Exit from Definition)

Exit a definition early with optional exit code:

```bash
# Early return
[
  "starting" echo
  0 return                   # Exit here with code 0
  "never reached" echo       # This won't run
] :early

# Return with exit code
[
  $1 -f test [0 return] [1 return] if
] :file_exists

# Check result
Cargo.toml file_exists      # exit code 0
missing.txt file_exists     # exit code 1
```

### Type and Which (Command Inspection)

Inspect how commands are interpreted:

```bash
# type - shows what a command is
"ls" type                    # ls is /bin/ls
"cd" type                    # cd is a shell builtin
"dup" type                   # dup is a hsab builtin
"myfunc" type                # myfunc is a hsab function

# which - similar but different format
"ls" which                   # /bin/ls
"cd" which                   # cd: shell builtin
```

## JSON Support

Parse and manipulate structured data:

```bash
# Parse JSON string to structured data
'{"name":"test","value":42}' json

# Convert back to JSON string
data unjson

# Parse JSON arrays
'[1,2,3]' json               # Stack: List([1, 2, 3])
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

Create temp files or named pipes from command output:

```bash
# subst: run command, push temp file path
[/dir1 ls] subst [/dir2 ls] subst diff  # diff <(ls /dir1) <(ls /dir2)

# fifo: like subst but uses named pipe (faster, no disk I/O)
[/dir1 ls] fifo [/dir2 ls] fifo diff
```

## Resource Limits

```bash
# timeout: kill command after N seconds
5 [10 sleep] timeout         # Killed after 5 seconds, exit code 124
```

## Pipeline Status

Get exit codes from all stages of a pipeline:

```bash
pipestatus                   # Returns list of exit codes from last pipeline
```

## Variables

Environment variables are expanded natively:

```bash
$HOME echo              # /home/user
$USER echo              # username
${PATH} echo            # Brace syntax also works
```

### Scoped Variable Assignment

Use semicolon to create temporary variable bindings that are restored after execution:

```bash
# Set variable for a single expression
ABC=5; $ABC echo                    # prints: 5

# Multiple assignments
A=hello B=world; $A $B echo         # prints: world hello (LIFO order)

# Shadowing - original value restored after scope
export MYVAR=original
MYVAR=temporary; $MYVAR echo        # prints: temporary
$MYVAR echo                         # prints: original (restored!)

# Without semicolon, treated as literal
ABC=5 echo                          # prints: ABC=5
```

This is useful for:
- Passing environment variables to commands without polluting the shell
- Temporary overrides that automatically clean up
- Script portability (no leftover variables)

## Comments

```bash
hello echo # this is ignored
ls # list files
```

Comments work inline and in scripts. They're stripped before parsing, respecting quotes:
```bash
"#not a comment" echo  # this IS a comment
```

## Quoting

Quotes preserve strings and prevent executable detection:

```bash
# Double quotes (content only, no surrounding quotes in value)
"hello world" echo      # hello world

# Single quotes (literal, content only)
'$HOME' echo            # $HOME (not expanded)

# Quote command names to use as args
"ls" echo               # ls (doesn't execute ls)
```

### Multiline Strings (Triple Quotes)

Use triple quotes for multiline text:

```bash
# Triple double-quotes
"""
line 1
line 2
line 3
""" echo

# Triple single-quotes (literal)
'''
$HOME stays literal
line 2
''' [cat] |
```

In the REPL, the prompt changes to `…` when entering multiline strings.

## Keyboard Shortcuts

The REPL provides keyboard shortcuts for efficient stack manipulation:

| Shortcut | Action |
|----------|--------|
| `Ctrl+O` | Pop from stack, insert value at start of input |
| `Alt+O` | Push first word from input to stack |
| `Ctrl+,` | Clear the entire stack |

The stack is displayed as a hint below your input line, updating in real-time as you use these shortcuts. The prompt shows `¢` when the stack has items, `£` when empty.

```bash
hsab-0.1.0£ hello world          # Empty stack
hsab-0.1.0£ foo bar              # Type something
                                 # Press Alt+O to push "foo"
hsab-0.1.0¢ bar                  # "foo" now on stack
 foo                             # Stack hint shown below
```

## Built-in REPL Commands

| Command | Description |
|---------|-------------|
| `.help` / `.h` | Show help message |
| `.stack` / `.s` | Show current stack |
| `.pop` / `.p` | Pop and show top value |
| `.clear` / `.c` | Clear the stack |
| `.use` / `.u` | Move top stack item to input line |
| `.use=N` / `.u=N` | Move N stack items to input line |
| `exit` / `quit` | Exit the REPL |

The stack persists between lines. Use `.use` to bring stack items into your next command:

```bash
hsab£ ls spread              # Files now on stack
hsab£ .s                     # See what's on stack
hsab£ .use=3                 # Move 3 items to input
hsab¢ file1 file2 file3 _    # Edit and complete command
```

## Script Files

Create `.hsab` files:

```bash
# example.hsab
# Lines starting with # are comments

# Simple commands
hello echo

# Pipe chains
ls [grep txt] |

# String operations
"/path/to/file.txt" "/" rsplit1 swap drop  # file.txt
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

## Standard Library

See `examples/stdlib.hsabrc` for a comprehensive collection of useful definitions including:

- **Path operations**: `dirname`, `basename`, `reext`
- **List operations**: `map`, `filter`, `dirs`, `files`
- **Control flow**: `when`, `unless`
- **Git shortcuts**: `gs`, `gd`, `gl`, `ga`, `gcm`
- **Navigation**: `ll`, `la`, `..`, `...`

To use the standard library, copy the definitions you want to `~/.hsabrc`:

```bash
# Copy the entire stdlib
cat examples/stdlib.hsabrc >> ~/.hsabrc

# Or copy specific definitions (e.g., just path operations)
grep -A1 ':dirname\|:basename\|:reext' examples/stdlib.hsabrc >> ~/.hsabrc
```

Or selectively copy individual definitions by opening `examples/stdlib.hsabrc` and pasting what you need into `~/.hsabrc`.

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
myfile.txt dup .bak suffix swap cp
# → cp myfile.txt myfile.txt.bak

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

hsab executes commands natively using Rust's `std::process::Command`:

1. **Native Execution**: Commands run directly without a bash subprocess
2. **Output Capture**: Command output is captured and pushed to the stack
3. **Synchronous by Default**: Commands block until complete
4. **Background (`&`)**: Spawns process without waiting, tracks as job
5. **Parallel**: Spawns threads, runs commands concurrently, collects output

### Automatic Interactive Detection

hsab automatically detects when to run commands interactively vs capture their output:

- **Interactive (output to terminal)**: When nothing will consume the output
- **Captured (output to stack)**: When piping, redirecting, or another command follows

```bash
ls                      # Runs interactively, output to terminal
pwd ls                  # pwd captured (ls consumes it), ls interactive
ls [grep Cargo] |       # ls captured (piped to grep)
```

This means `vim`, `less`, `python3` etc. "just work" without special handling.

## How Words Are Classified

When hsab encounters a word, it must decide whether to **execute** it (command) or **push** it (literal). Here's how it works:

### Detection Rules

1. **Shell builtins**: hsab's own builtins (`cd`, `pwd`, `echo`, `test`, `true`, `false`, etc.) are always recognized
2. **PATH lookup**: Words found in your `$PATH` directories are treated as commands
3. **User definitions**: Words defined with `:name` are executed as defined
4. **Everything else**: Words not matching the above are pushed as literals

### Examples

```bash
hello echo           # "hello" is literal (not in PATH), "echo" is builtin
mydata process       # If "process" is in PATH, it runs as a command
```

### Forcing Literals with Quotes

To force a word to be a literal (never executed), quote it:

```bash
"file" -f test       # "file" is now a literal, not the file command
"yes" echo           # Push "yes" as string, then echo it
```

### Control Flow and Quoting

**Important**: Inside control flow blocks (`if`, `while`, `times`), always quote string literals that might match PATH commands:

```bash
# WRONG - "yes" is /usr/bin/yes which runs forever!
[true] [yes echo] [no echo] if

# CORRECT - quoted strings are always literals
[true] ["yes" echo] ["no" echo] if

# CORRECT - "x" is not a command, no quotes needed
3 [x echo] times
```

### Best Practices

1. **Quote strings in control flow**: Always quote literal strings in `if`/`while`/`times` blocks
2. **When in doubt, quote it**: If a word might match a command, quote it
3. **Check your PATH**: Run `which <word>` to see if something is a command
4. **Use definitions**: Create named commands to avoid ambiguity: `["my-cmd"] :mycmd`

### Common Gotchas

Many common English words are also Unix commands:

```bash
# These words exist in PATH and will execute:
yes                  # Outputs "y" forever - always quote: "yes"
time                 # Times command execution
file                 # Determines file type
test                 # Evaluates expressions (hsab builtin)
more                 # Pagination
less                 # Pagination
head                 # First lines of file
tail                 # Last lines of file
sort                 # Sorts input
cut                  # Extracts columns
join                 # Joins files
split                # Splits files
```

When using these as data, always quote them: `"yes"`, `"time"`, `"file"`, etc.

## Design Philosophy

hsab is built on these principles:

1. **Stack semantics**: Data flows through a stack, commands pop and push
2. **Output threading**: Command output automatically becomes input for the next
3. **Deferred execution**: Blocks `[...]` are first-class values
4. **Explicit control**: Operators like `@` `|` `&&` make data flow visible
5. **Standalone**: No bash dependency - pure Rust execution

## Tips

1. **Think in stacks**: Values push, commands pop
2. **Use blocks for grouping**: `[cmd args]` defers execution
3. **Quote command names**: `"ls" echo` to use as string
4. **Apply for execution**: `[cmd] @` runs a block
5. **Pipes for data flow**: `cmd [consumer] |`
6. **Stack ops for reordering**: `swap`, `dup`, `over`
7. **Postfix test syntax**: `file.txt -f test` not `test -f file.txt`

## License

MIT
