# Getting Started with hsab

Welcome to **hsab** (bash spelled backwards), a stack-based shell with postfix notation. If you are coming from bash or zsh, this guide will help you understand the core concepts and get productive quickly.

## What Makes hsab Different?

In traditional shells, commands flow left-to-right with arguments:
```bash
# bash
cat file.txt | grep error | wc -l
```

In hsab, operations come *after* their operands (postfix notation), and results stay on a **stack**:
```bash
# hsab
file.txt cat "error" grep -l wc
```

The big advantage: intermediate results persist on the stack. You can inspect them, backtrack, and refine without retyping entire pipelines.

---

## Installation

hsab is written in Rust. Install it from source:

```bash
# Clone the repository
git clone https://github.com/yourusername/hsab.git
cd hsab

# Build and install
cargo build --release
cp target/release/hsab /usr/local/bin/

# Initialize the standard library
hsab init
```

Verify the installation:
```bash
hsab --version
```

Start the interactive REPL:
```bash
hsab
```

You should see a prompt (the default shows your current directory). Type `exit` or press Ctrl+D to quit.

---

## Your First Commands

### Basic Arithmetic

Let's start simple. In hsab, you push values onto the stack, then apply operations:

```bash
> 2 3 +
5
```

What happened?
1. `2` pushed the number 2 onto the stack
2. `3` pushed the number 3 onto the stack
3. `+` popped both numbers, added them, and pushed the result (5)

More examples:
```bash
> 10 3 -
7

> 4 5 *
20

> 20 4 /
5

> 17 5 mod
2
```

You can chain operations:
```bash
> 2 3 + 4 *
20
```

This computes `(2 + 3) * 4`. The postfix order means you never need parentheses.

### Running Shell Commands

hsab runs shell commands just like bash:
```bash
> ls
Cargo.toml  src  target  README.md

> echo "Hello, world!"
Hello, world!

> pwd
/home/user/projects
```

Commands with flags work normally:
```bash
> ls -la
> git status
> docker ps
```

---

## Understanding the Stack

The stack is hsab's central concept. Values you type are pushed onto it; operations consume values and push results.

### Viewing the Stack

Use `.s` to see what is on the stack:
```bash
> 1 2 3
> .s
[1, 2, 3]
```

Use `depth` to see how many items are on the stack:
```bash
> 1 2 3
> depth
3
```

### Stack Manipulation

These operations rearrange the stack without modifying values:

| Operation | Before | After | Description |
|-----------|--------|-------|-------------|
| `dup`     | a      | a a   | Duplicate top item |
| `drop`    | a b    | a     | Remove top item |
| `swap`    | a b    | b a   | Swap top two items |
| `over`    | a b    | a b a | Copy second item to top |
| `rot`     | a b c  | b c a | Rotate third item to top |

Examples:
```bash
> 5 dup
> .s
[5, 5]

> 1 2 swap
> .s
[2, 1]

> 10 20 30 rot
> .s
[20, 30, 10]

> "hello" "world" drop
> .s
["hello"]
```

### Clearing the Stack

Press `Alt+k` to clear the entire stack, or use `drop` repeatedly.

---

## The Postfix Mindset

Coming from bash, you might write:
```bash
# bash
wc -l file.txt
```

In hsab, the file comes first, then the command:
```bash
# hsab
file.txt -l wc
```

Think of it as: "Take file.txt, count its lines."

### Why Postfix?

1. **No parentheses needed** - Order of operations is always left-to-right
2. **Easy composition** - Chain operations without pipes or subshells
3. **Persistent results** - Intermediate values stay on the stack

Compare these equivalent operations:

| bash | hsab |
|------|------|
| `echo "hello"` | `"hello" echo` |
| `cat file.txt` | `file.txt cat` |
| `grep -r "TODO" .` | `. "TODO" -r grep` |
| `wc -l $(cat file.txt)` | `file.txt cat -l wc` |

### A Mental Model

Think of the stack as a conveyor belt. Values enter from the left. Operations grab what they need from the right (top of stack), process it, and put results back.

```
Push 5:     [5]
Push 3:     [5, 3]
Add:        [8]        (consumed 5 and 3, pushed 8)
Push 2:     [8, 2]
Multiply:   [16]       (consumed 8 and 2, pushed 16)
```

---

## Running Shell Commands

### Direct Execution

Most shell commands work exactly as you would expect:
```bash
> ls -la
> git status
> docker ps -a
> curl -s https://example.com
```

### Commands with Arguments from Stack

When you run a command, hsab checks if it needs arguments. If the stack has values, they can be used:
```bash
> "hello world" echo
hello world

> README.md cat
(contents of README.md)
```

### Quoting

Strings with spaces need quotes:
```bash
> "Hello, world!" echo
Hello, world!

> '/path/with spaces/file.txt' cat
```

Both single and double quotes work. Double quotes allow variable expansion.

---

## Capturing Output with slurp

The `slurp` command captures a command's output as a single string on the stack:
```bash
> pwd slurp
> .s
["/home/user/projects"]
```

Without `slurp`, commands print to the terminal but do not push results to the stack.

### Using Captured Output

Capture output, then use it:
```bash
> date slurp
> "Today is: " swap suffix
> echo
Today is: Sat Feb 14 10:30:00 PST 2026
```

### spread: Lines to Stack Items

Use `spread` to split output into individual stack items:
```bash
> ls spread
> .s
["file1.txt", "file2.txt", "file3.txt"]
```

This is powerful for processing files one by one:
```bash
> *.txt ls spread        # Each .txt file is now a stack item
> .s                     # Inspect
> [-l wc] each           # Count lines in each
```

---

## Simple Pipelines

### Traditional Pipe Style

hsab supports the familiar `|` pipe:
```bash
> ls | grep ".txt"
file1.txt
notes.txt
```

### Stack-Based Approach

The stack-based alternative often reads more naturally:
```bash
> ls spread [".txt" ends?] keep
```

This:
1. Lists files and spreads them onto the stack
2. Keeps only items ending with ".txt"

### The each Operation

Apply an operation to every item:
```bash
> ls spread [-l wc] each
```

This runs `wc -l` on each file, pushing all results.

### Filtering with keep

Keep items matching a condition:
```bash
> 1 2 3 4 5 6 7 8 9 10
> [2 mod 0 eq?] keep    # Keep even numbers
> .s
[2, 4, 6, 8, 10]
```

For files:
```bash
> ls spread [-f test] keep   # Keep only regular files (not directories)
```

---

## Blocks and Applying Them

Blocks are pieces of code enclosed in `[ ]`:
```bash
["hello" echo]
```

A block is a value. It sits on the stack until you apply it.

### Applying Blocks with @

The `@` operator executes a block:
```bash
> ["Hello!" echo] @
Hello!
```

### Storing Blocks as Definitions

Create reusable commands with `:name`:
```bash
> [-la ls] :ll
> ll
(detailed file listing)
```

Now `ll` is available for the rest of your session. Add it to `~/.hsabrc` to make it permanent.

### Blocks in Control Flow

Blocks power conditionals and loops:
```bash
# If/else
> 5 3 gt? ["bigger" echo] ["smaller" echo] if
bigger

# Loop N times
> 3 ["Hello!" echo] times
Hello!
Hello!
Hello!
```

---

## Practical Examples

### Example 1: Find Large Files

```bash
> *.log ls spread                    # All .log files
> [du -h] each                       # Get sizes
> .s                                 # Inspect results
```

### Example 2: Git Workflow

```bash
> status git                         # Check status
> *.rs ls spread [add git] each      # Stage all Rust files
> "Fix bug" -m commit git            # Commit
```

### Example 3: Process Text Files

```bash
> notes.txt cat                      # Read file
> "TODO" grep                        # Find TODOs
```

### Example 4: Interactive Exploration

This is where hsab shines:
```bash
> *.txt ls spread           # Start with all .txt files
> .s                        # See what we have (15 files)
> [".bak" ends?] reject     # Remove backup files
> .s                        # Now 12 files
> [-mtime -7 test] keep     # Only files modified in last week
> .s                        # Down to 4 files
> [cat] each                # Read them all
```

At any step, you can inspect with `.s`, backtrack with `drop`, or try a different filter.

---

## Keyboard Shortcuts

These make interactive use faster:

| Shortcut | Action |
|----------|--------|
| **Alt+k** | Clear the stack |
| **Alt+c** | Copy top of stack to clipboard |
| **Alt+x** | Cut top of stack to clipboard |

**Note:** On macOS, configure your terminal to use Option as Meta:
- iTerm2: Preferences, Profiles, Keys, "Option key acts as: Esc+"
- Terminal.app: Preferences, Profiles, Keyboard, "Use Option as Meta key"

---

## REPL Enhancements

hsab includes optional features to make the interactive experience more pleasant.

### Syntax Highlighting

Enable colorized input as you type:

```bash
# Add to ~/.bashrc or ~/.zshrc
export HSAB_HIGHLIGHT=1
```

Or toggle at runtime:
```
hsab> .highlight
Syntax highlighting: ON
```

Colors help you distinguish:
- **Builtins** (blue): `echo`, `dup`, `map`
- **Strings** (green): `"hello"`, `'text'`
- **Numbers** (yellow): `42`, `3.14`
- **Blocks** (magenta): `[echo hello]`
- **Variables** (cyan): `$HOME`, `$name`

### History Suggestions

Enable fish-style inline suggestions from your command history:

```bash
# Add to ~/.bashrc or ~/.zshrc
export HSAB_SUGGESTIONS=1
```

Or toggle at runtime:
```
hsab> .suggestions
History suggestions: ON
```

As you type, matching history entries appear dimmed. Press **Right Arrow** to accept.

### Recommended Setup

For the best interactive experience, add to your shell profile:

```bash
# ~/.bashrc or ~/.zshrc
export HSAB_HIGHLIGHT=1
export HSAB_SUGGESTIONS=1
```

See [Configuration Guide](config.md#hsab_highlight) for all options.

---

## Next Steps

Now that you understand the basics:

1. **Master the REPL** - See [Interactive REPL Guide](repl.md) for shortcuts, debugging, and workflows
2. **Explore builtins** - Run `hsab --help` for the complete reference
3. **Coming from bash?** - See [Migration Guide](migration.md) for side-by-side patterns
4. **Compare shells** - See [Comparison](comparison.md) for hsab vs bash, fish, zsh, nushell
5. **Customize your environment** - See [Configuration Guide](config.md)
6. **Learn file operations** - See [Shell Guide: Stack-Native Operations](shell.md#stack-native-shell-operations)
7. **Create your own commands** - See [Extending the Standard Library](extending-stdlib.md)

### Quick Reference

| Concept | Example |
|---------|---------|
| Push values | `1 2 3` |
| Arithmetic | `2 3 +` gives 5 |
| View stack | `.s` |
| Duplicate | `dup` |
| Remove top | `drop` |
| Run command | `ls -la` |
| Postfix command | `file.txt cat` |
| Capture output | `pwd slurp` |
| Spread lines | `ls spread` |
| Apply to each | `[wc -l] each` |
| Filter | `[pred?] keep` |
| Define command | `[-la ls] :ll` |
| Apply block | `[code] @` |

Welcome to hsab. Think in stacks, compose without pipes, and enjoy the persistent state.
