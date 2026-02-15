# Interactive REPL Guide

The hsab REPL is designed for **exploratory interaction**. Unlike traditional shells where each command starts fresh, hsab's persistent stack lets you build up results, inspect them, backtrack, and refine — all without losing your intermediate work.

This guide covers everything that makes hsab's interactive experience unique.

**See also:**
- [Getting Started](getting-started.md) - Core concepts and first commands
- [Customizing Prompts](customizing-prompts.md) - Deep dive into prompt configuration
- [Configuration](config.md) - Environment variables and settings
- [Reference](reference.md) - Complete language reference

---

## Table of Contents

1. [The Exploration Workflow](#the-exploration-workflow)
2. [Visual Stack Hint](#visual-stack-hint)
3. [Stack Manipulation Shortcuts](#stack-manipulation-shortcuts)
4. [Limbo: Pending Input](#limbo-pending-input)
5. [Syntax Highlighting](#syntax-highlighting)
6. [History Suggestions](#history-suggestions)
7. [Debugging and Stepping](#debugging-and-stepping)
8. [REPL Commands](#repl-commands)
9. [Clipboard Integration](#clipboard-integration)
10. [Keyboard Reference](#keyboard-reference)

---

## The Exploration Workflow

This is hsab's core interaction pattern: **push, inspect, refine, backtrack, continue**.

### The Problem with Traditional Shells

In bash, every refinement restarts from scratch:

```bash
$ ls -1
$ ls -1 | grep -v /
$ ls -1 | grep -v / | grep '\.rs$'
$ ls -1 | grep -v / | grep '\.rs$' | xargs wc -l
```

You retype the entire pipeline each time. If you want to try a different filter, you start over.

### The hsab Way: Incremental Exploration

```bash
> *.rs ls spread              # Push all .rs files to stack
> .s                          # INSPECT: see what we have
["main.rs", "lib.rs", "eval.rs", "parser.rs", "lexer.rs"]

> ["test" contains?] keep     # REFINE: filter to test files
> .s
["test_eval.rs", "test_parser.rs"]

# Hmm, I wanted the opposite - let me backtrack
> drop drop                   # BACKTRACK: remove filtered results
> *.rs ls spread              # Start fresh with all files
> ["test" contains?] reject   # REFINE differently: exclude tests
> .s
["main.rs", "lib.rs", "eval.rs", "parser.rs", "lexer.rs"]

> [wc -l] each                # CONTINUE: count lines in each
```

### Key Techniques

| Technique | Commands | Purpose |
|-----------|----------|---------|
| **Push** | `ls spread`, `*.txt`, values | Get data onto the stack |
| **Inspect** | `.s`, `depth`, `dup .s drop` | See what you have |
| **Refine** | `keep`, `reject`, `each`, `sort` | Transform/filter stack contents |
| **Backtrack** | `drop`, `Alt+k`, undo | Remove unwanted results |
| **Continue** | Any operation | Build on current stack |

### Real Session: Finding Large Log Files

```bash
> /var/log ls spread           # All files in /var/log
> .s                           # 47 files - too many
> [".log" ends?] keep          # Just .log files
> .s                           # 23 files
> [stat -f %z] each            # Get sizes
> .s                           # Now have sizes on stack
# Hmm, I want filename + size pairs...
> Alt+k                        # Clear and try again

> /var/log ls spread
> [dup stat -f %z] each        # Keep filename, add size
> [1000000 gt?] keep           # Filter to files > 1MB
> .s                           # Large log files with sizes
```

The stack holds your working set. You sculpt it incrementally.

---

## Visual Stack Hint

As you type, hsab shows a preview of the current stack state in your prompt or status area. This real-time feedback is unique to hsab.

### How It Works

The stack hint updates after every operation:

```
[3] main.rs lib.rs eval.rs > _
```

This shows:
- `[3]` - Three items on stack
- `main.rs lib.rs eval.rs` - Preview of top items
- `>` - Your prompt
- `_` - Cursor position

### Configuration

The stack hint is part of prompt customization. See [Customizing Prompts](customizing-prompts.md) for full details.

Quick setup in `~/.hsabrc`:

```hsab
# Show stack depth in prompt
{ "[" depth str + "] > " + } '_PS1' def

# Show top item preview
{
  depth 0 >
  { "[" dup str + "] " + }
  { "" }
  if
  "hsab> " +
} '_PS1' def
```

### Why It Matters

In traditional shells, you can't see intermediate state. You run a command, see output, run another. The stack hint lets you:

1. **Verify before executing** - See what's on stack before applying operations
2. **Catch mistakes early** - Notice wrong values before they propagate
3. **Build intuition** - Learn how operations affect the stack

---

## Stack Manipulation Shortcuts

These keyboard shortcuts let you move data between the stack and your input line, enabling fluid interactive workflows.

### Core Shortcuts

| Shortcut | Action | Description |
|----------|--------|-------------|
| **Alt+↑** | Pop to input | Pop top of stack into input line (as limbo ref) |
| **Alt+↓** | Push to stack | Push first word from input onto stack |
| **Alt+A** | Push all | Push ALL words from input onto stack |
| **Alt+a** | Pop all | Pop ALL stack items into input |
| **Alt+k** | Clear stack | Clear the entire stack |
| **Ctrl+O** | Pop to input | Alternative pop binding (terminal compatibility) |

### Workflow: Building Commands Incrementally

```bash
# Type a path, push it to stack for later
> /var/log/app.log            # Type the path
> Alt+↓                       # Push to stack, input clears
> cat                         # Now type the command
> Alt+↑                       # Pop path back: "cat /var/log/app.log"
> Enter                       # Execute
```

### Workflow: Saving Results for Later

```bash
> pwd                         # Get current directory
> Alt+↓                       # Push it to stack
> cd /tmp                     # Work elsewhere
> ls                          # Do things...
> Alt+↑                       # Pop saved directory to input
> cd                          # Return: "cd /original/path"
```

### Workflow: Reusing Output

```bash
> find . -name "*.rs" spread  # Find files, spread to stack
> .s                          # Inspect results
> Alt+↑                       # Pop one file into input
> vim                         # Edit it: "vim src/main.rs"
> Enter
# After editing, remaining files still on stack
> Alt+↑                       # Pop next file
> vim                         # Edit another
```

### Terminal Setup (macOS)

For Alt shortcuts to work on macOS:

**iTerm2:**
Preferences → Profiles → Keys → "Option key acts as: Esc+"

**Terminal.app:**
Preferences → Profiles → Keyboard → "Use Option as Meta key"

---

## Limbo: Pending Input

Limbo is a staging area between the stack and your input line. When you pop a complex value with Alt+↑, it enters "limbo" — referenced in your input but preserved until execution.

### Simple vs Complex Values

When you pop a value, hsab decides how to insert it:

- **Simple values** (numbers, bools, short strings): Inserted directly as text
- **Complex values** (records, tables, long strings, blocks): Inserted as a **limbo reference**

```bash
> 42 "hello"                  # Two simple values on stack
> Alt+↑                       # Pop "hello" - inserted directly
> echo hello                  # Simple string inserted as-is

> {"name" "alice"} record     # Complex value (record) on stack
> Alt+↑                       # Pop record - creates limbo reference
> `&0001:record:{name}`       # Limbo reference appears
```

### Limbo Reference Format

Limbo references use backtick syntax with an `&` prefix:

```
`&id:type:preview`
```

- `&` - Identifies this as a limbo reference
- `id` - Unique 4-digit hex ID (e.g., `0001`, `0002`)
- `type` - Value type hint (e.g., `string`, `record`, `vector[25]`)
- `preview` - Value preview (truncated for large values)

**Examples:**
```bash
`&0001:string[156]:"The quic..."`   # Long string with length
`&0002:i64:42`                       # Integer
`&0003:record:{name, age}`           # Record with field names
`&0004:vector[25]:[1, 2, 3]`         # Vector with length and preview
`&0005:table[5x10]`                  # Table with dimensions
`&0006:block:[...]`                  # Block
```

### Limbo Behavior

| Action | Result |
|--------|--------|
| **Enter** (execute) | Refs resolve to actual values, limbo clears |
| **Ctrl+C** (cancel) | Limbo values return to stack, nothing lost |
| **Ctrl+U** (clear line) | Limbo values return to stack, input cleared |
| **Edit the ref** | You can modify the text; only the ID matters for resolution |
| **Invalid ID** | Resolves to `nil` (graceful degradation) |

### Why Limbo Matters

1. **Non-destructive editing** - Pop values to edit them, cancel safely with Ctrl+C
2. **Visual clarity** - See what's pending vs. committed in your input
3. **Type preservation** - Complex values (tables, records, blocks) keep their types
4. **Composability** - Reference multiple values in one command: `` `&0001` `&0002` swap ``

---

## Syntax Highlighting

hsab colorizes your input as you type, making code structure immediately visible.

### Enabling

```bash
# In your shell profile (~/.bashrc or ~/.zshrc)
export HSAB_HIGHLIGHT=1

# Or toggle at runtime
hsab> .highlight
Syntax highlighting: ON
```

### Color Scheme

| Token Type | Color | Examples |
|------------|-------|----------|
| **Builtins** | Blue | `dup`, `swap`, `map`, `if`, `each` |
| **Strings** | Green | `"hello"`, `'text'` |
| **Numbers** | Yellow | `42`, `3.14`, `-17` |
| **Blocks** | Magenta | `[echo hello]`, `[dup *]` |
| **Operators** | Cyan | `@`, `\|`, `:`, `&&`, `\|\|` |
| **Variables** | Cyan | `$HOME`, `$name` |
| **Comments** | Gray | `# this is a comment` |
| **Definitions** | Bold | User-defined words |

### When to Disable

Toggle off with `.highlight` when:
- Pasting large code blocks (can slow input)
- Terminal colors are hard to read
- Copying output (ANSI codes may interfere)

See [Configuration: HSAB_HIGHLIGHT](config.md#hsab_highlight) for details.

---

## History Suggestions

Fish-style inline suggestions show matching commands from your history as you type.

### Enabling

```bash
# In your shell profile
export HSAB_SUGGESTIONS=1

# Or toggle at runtime
hsab> .suggestions
History suggestions: ON
```

### How It Works

As you type, hsab searches history for matching entries:

```
hsab> ec                      # You type "ec"
hsab> ec→ho "hello world"     # Suggestion appears dimmed
```

The suggestion `→ho "hello world"` appears after an arrow character. Press **Right Arrow** to accept.

### Accepting Suggestions

| Key | Action |
|-----|--------|
| **Right Arrow** / **End** | Accept full suggestion |
| **Tab** | Accept next word only |
| **Ctrl+E** | Accept full suggestion |
| **Keep typing** | Ignore suggestion |

### Customizing the Arrow

```bash
export HSAB_SUGGESTION_ARROW="→"    # Default
export HSAB_SUGGESTION_ARROW=" -> " # ASCII alternative
export HSAB_SUGGESTION_ARROW=""     # No arrow
```

See [Configuration: HSAB_SUGGESTIONS](config.md#hsab_suggestions) for details.

---

## Debugging and Stepping

hsab has built-in debugging tools for understanding execution flow.

### Debug Mode

Toggle debug mode to see detailed execution:

```bash
hsab> .debug
Debug mode: ON

hsab> 2 3 +
[DEBUG] Push: 2
[DEBUG] Push: 3
[DEBUG] Apply: +
[DEBUG] Result: 5
5

hsab> .d              # Short form to toggle off
Debug mode: OFF
```

### Step Mode

Step through execution one operation at a time:

```bash
hsab> .step
Step mode: ON

hsab> 1 2 3 + *
[STEP] 1 -> Press Enter...
[STEP] 2 -> Press Enter...
[STEP] 3 -> Press Enter...
[STEP] + -> Press Enter...
[STEP] * -> Press Enter...
9
```

### Breakpoints

Set breakpoints on specific words:

```bash
hsab> .break myfunction
Breakpoint set on: myfunction

hsab> data myfunction process
[BREAK] myfunction - Stack: [data]
Continue? (y/n/s for step): _
```

### Debug Commands

| Command | Short | Action |
|---------|-------|--------|
| `.debug` | `.d` | Toggle debug mode |
| `.step` | | Enable step-by-step mode |
| `.break <pattern>` | `.b` | Set breakpoint on pattern |
| `.delbreak <pattern>` | `.db` | Remove breakpoint |
| `.breakpoints` | `.bl` | List all breakpoints |
| `.clearbreaks` | `.cb` | Clear all breakpoints |

### Interactive Debug Commands (when paused)

| Key | Action |
|-----|--------|
| `n` / Enter | Step to next expression |
| `c` | Continue until next breakpoint |
| `s` | Show current stack |
| `b` | List breakpoints |
| `q` | Quit debug mode |

---

## REPL Commands

Dot-commands control the REPL environment. They affect shell state rather than data.

### Stack Inspection

| Command | Short | Action |
|---------|-------|--------|
| `.s` | `.stack` | Display stack contents |
| `depth` | | Push stack depth |
| `.clear` | | Clear terminal screen |

### Mode Toggles

| Command | Short | Action |
|---------|-------|--------|
| `.debug` | `.d` | Toggle debug mode |
| `.step` | | Toggle step mode |
| `.highlight` | `.hl` | Toggle syntax highlighting |
| `.suggestions` | `.sug` | Toggle history suggestions |

### Help

| Command | Action |
|---------|--------|
| `.help` | Show available commands |
| `"topic" help` | Get help on specific topic |
| `words` | List all defined words |

---

## Clipboard Integration

hsab integrates with your system clipboard via OSC 52 terminal sequences.

### Commands

| Command | Stack Effect | Description |
|---------|--------------|-------------|
| `.copy` | value → value | Copy top to clipboard (non-destructive) |
| `.cut` | value → | Copy and remove from stack |
| `.paste` | → value | Paste from clipboard onto stack |
| `paste-here` | → value | Inline clipboard expansion |

### Shortcuts

| Shortcut | Action |
|----------|--------|
| **Alt+c** | Copy top of stack to clipboard |
| **Alt+x** | Cut top of stack to clipboard |

### Examples

```bash
# Copy a result for use elsewhere
> pwd .copy                   # Copy current directory
> # Switch to another app, paste there

# Paste into hsab
> .paste                      # Clipboard contents now on stack
> cat                         # Use as argument

# Inline expansion
> paste-here .bak suffix      # "clipboard-value.bak"
```

### Terminal Support

OSC 52 works in: iTerm2, Kitty, tmux (with `set-clipboard on`), and most modern terminals.

---

## Keyboard Reference

Complete keyboard shortcut reference for interactive use.

### Stack Manipulation

| Shortcut | Action |
|----------|--------|
| **Alt+↑** | Pop top of stack to input (as limbo ref for complex values) |
| **Alt+↓** | Push first word to stack |
| **Alt+A** | Push ALL words to stack |
| **Alt+a** | Pop ALL stack items to input |
| **Alt+k** | Clear entire stack |
| **Ctrl+O** | Pop to input (alternative binding) |

### Clipboard

| Shortcut | Action |
|----------|--------|
| **Alt+c** | Copy top to system clipboard |
| **Alt+x** | Cut top to system clipboard |

### History Navigation

| Shortcut | Action |
|----------|--------|
| **↑** / **Ctrl+P** | Previous command |
| **↓** / **Ctrl+N** | Next command |
| **Ctrl+R** | Reverse incremental search |

### Line Editing

| Shortcut | Action |
|----------|--------|
| **Ctrl+A** | Move to beginning of line |
| **Ctrl+E** | Move to end of line |
| **Ctrl+U** | Clear line and return limbo values to stack |
| **Ctrl+K** | Delete to end of line |
| **Ctrl+W** | Delete word backward |

### Suggestions (when enabled)

| Shortcut | Action |
|----------|--------|
| **→** / **End** | Accept full suggestion |
| **Tab** | Accept next word |
| **Keep typing** | Ignore suggestion |

### Session Control

| Shortcut | Action |
|----------|--------|
| **Ctrl+D** | Exit (on empty line) |
| **Ctrl+C** | Cancel current input |
| **Ctrl+L** | Clear screen |

---

## Next Steps

- [Customizing Prompts](customizing-prompts.md) - Deep dive into prompt configuration
- [Configuration](config.md) - All environment variables and settings
- [Shell Guide](shell.md) - Stack-native shell operations
- [Reference](reference.md) - Complete language reference
