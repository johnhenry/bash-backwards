# Customizing hsab Prompts

hsab provides three customizable prompts: PS1 (main prompt), PS2 (continuation), and STACK_HINT (stack preview). This guide shows you how to personalize them.

## Quick Start

Add these to your `~/.hsabrc` (personal config) or `~/.hsab/lib/stdlib.hsabrc` (stdlib):

```bash
# Simple prompt showing just the symbol
["hsab> "] :PS1

# Or with version and stack indicator
[
  "hsab-" $_VERSION suffix
  [$_DEPTH 0 gt?] ["* "] ["> "] if suffix
] :PS1
```

## The Three Prompts

### PS1 - Main Prompt

Displayed before each command. Must return a string on the stack.

```bash
# Default: shows version, £ when empty, ¢ when stack has items
[
  "hsab-" $_VERSION suffix
  [$_DEPTH 0 gt?] ["¢ "] ["£ "] if suffix
] :PS1
```

### PS2 - Continuation Prompt

Displayed when input spans multiple lines (unclosed brackets/quotes).

```bash
# Default: shows version with ellipsis
["hsab-" $_VERSION "… " suffix suffix] :PS2

# Simpler alternative
["... "] :PS2
```

### STACK_HINT - Stack Preview

Formats the stack preview shown above the prompt. Receives stack items as a newline-separated string.

```bash
# Default: items separated by spaces
["\n" " " str-replace] :STACK_HINT

# Show as vertical list with bullets
["\n" "\n• " str-replace "• " swap suffix] :STACK_HINT

# Show count and top item only
[
  "\n" split1 drop
  _TOP local
  depth " items, top: " $_TOP suffix suffix
] :STACK_HINT
```

## Available Context Variables

These variables are available in your prompt definitions:

### Version Info
| Variable | Description | Example |
|----------|-------------|---------|
| `$_VERSION` | Full version string | `"0.1.0"` |
| `$_VERSION_MAJOR` | Major version | `"0"` |
| `$_VERSION_MINOR` | Minor version | `"1"` |
| `$_VERSION_PATCH` | Patch version | `"0"` |

### Shell State
| Variable | Description | Example |
|----------|-------------|---------|
| `$_DEPTH` | Stack depth | `"3"` |
| `$_EXIT` | Last exit code | `"0"` |
| `$_JOBS` | Background job count | `"2"` |
| `$_CMD_NUM` | Command number | `"42"` |
| `$_SHLVL` | Shell nesting level | `"1"` |

### Environment
| Variable | Description | Example |
|----------|-------------|---------|
| `$_CWD` | Current directory | `"/home/user/proj"` |
| `$_USER` | Username | `"john"` |
| `$_HOST` | Hostname | `"macbook"` |
| `$_TIME` | Current time | `"14:30:45"` |
| `$_DATE` | Current date | `"2024-01-15"` |

### Git Info (when in a repo)
| Variable | Description | Example |
|----------|-------------|---------|
| `$_GIT_BRANCH` | Current branch | `"main"` |
| `$_GIT_DIRTY` | Uncommitted changes? | `"1"` or `"0"` |
| `$_GIT_REPO` | Repository name | `"hsab"` |

## Example Prompts

### Minimal

```bash
["$ "] :PS1
```

### With Username and Directory

```bash
[
  $_USER "@" suffix
  $_HOST ":" suffix suffix
  $_CWD " $ " suffix suffix
] :PS1
# Output: john@macbook:/home/user/proj $
```

### Git-Aware

```bash
[
  $_CWD " " suffix
  [$_GIT_BRANCH len 0 gt?] [
    "(" $_GIT_BRANCH suffix
    [$_GIT_DIRTY "1" eq?] ["*" suffix] [] if
    ")" suffix
    " " suffix
  ] [] if
  "$ " suffix
] :PS1
# Output: /home/user/proj (main*) $
```

### Stack-Aware with Colors

```bash
[
  # ANSI colors (if your terminal supports them)
  "\033[32m" "hsab" suffix "\033[0m" suffix  # green "hsab"
  "-" $_VERSION suffix
  " " suffix
  [$_DEPTH 0 gt?] [
    "\033[33m" $_DEPTH suffix "\033[0m" suffix  # yellow count
    " items " suffix
  ] [] if
  [$_EXIT "0" ne?] [
    "\033[31m" "!" suffix "\033[0m " suffix  # red ! for errors
  ] [] if
  "> " suffix
] :PS1
```

### Show Exit Code on Failure

```bash
[
  [$_EXIT "0" ne?] [
    "[" $_EXIT "]" suffix suffix " " suffix
  ] [""] if
  "$ " suffix
] :PS1
# Output: [1] $  (after failed command)
# Output: $      (after successful command)
```

### Time-Based

```bash
[
  "[" $_TIME "] " suffix suffix
  $_CWD " $ " suffix suffix
] :PS1
# Output: [14:30:45] /home/user $
```

### Two-Line Prompt

```bash
[
  $_CWD "\n" suffix suffix
  [$_DEPTH 0 gt?] ["(" $_DEPTH " items) " suffix suffix] [] if
  "$ " suffix
] :PS1
# Output:
# /home/user/proj
# (3 items) $
```

## Stack Hint Examples

### Compact (Default)

```bash
["\n" " " str-replace] :STACK_HINT
# Shows: foo bar baz
```

### Numbered

```bash
[
  "\n" split spread
  _IDX local 1 _IDX local
  [
    "[" $_IDX "]" suffix suffix ": " suffix swap suffix
    $_IDX 1 plus _IDX local
  ] each
  "\n" str-join
] :STACK_HINT
# Shows:
# [1]: foo
# [2]: bar
# [3]: baz
```

### Truncated

```bash
[
  # Only show if 3 or fewer items
  dup "\n" indexof -1 gt?
  dup "\n" split1 drop swap
  # ... complex truncation logic
  "\n" " | " str-replace
] :STACK_HINT
```

### Type-Annotated

```bash
# Show type hints for structured data
[
  "\n" " " str-replace
  # Types would need runtime detection
  # This is a simplified example
] :STACK_HINT
```

## Tips and Tricks

### Keep It Fast

Prompts run before every command. Avoid slow operations:

```bash
# BAD: runs git on every prompt
[status git | head -1] :PS1

# GOOD: use cached variables
[$_GIT_BRANCH] :PS1
```

### Test Before Committing

Test your prompt interactively:

```bash
hsab
£ ["TEST> "] :PS1
TEST> # see if you like it
TEST> # then add to ~/.hsabrc
```

### Fallback for Missing Variables

Handle cases where variables might be empty:

```bash
[
  [$_GIT_BRANCH len 0 gt?]
  ["(" $_GIT_BRANCH ")" suffix suffix]
  [""]
  if
  " $ " suffix
] :PS1
```

### Unicode Symbols

Modern terminals support Unicode:

```bash
# Fancy symbols
["λ "] :PS1
["→ "] :PS1
["❯ "] :PS1
["▶ "] :PS1

# With stack indicator
[[$_DEPTH 0 gt?] ["◆ "] ["◇ "] if] :PS1
```

### Match Your Shell

Make hsab feel familiar:

```bash
# Bash-like
[$_USER "@" $_HOST suffix suffix ":" suffix $_CWD suffix "$ " suffix] :PS1

# Zsh-like
[$_CWD " %% " suffix] :PS1

# Fish-like
[$_USER "@" suffix $_CWD " > " suffix suffix] :PS1
```

## Troubleshooting

**Prompt shows nothing?**
- Ensure your PS1 leaves a string on the stack
- Test with: `["test> "] :PS1`

**Prompt has weird characters?**
- Check for unescaped special characters
- Verify terminal supports your Unicode/ANSI codes

**Prompt is slow?**
- Remove any external command calls
- Use `$_GIT_*` variables instead of running `git`

**Stack hint not showing?**
- STACK_HINT only shows when stack has items
- Push something: `hello` then check

## Sharing Your Prompts

Found a great prompt configuration? Add it to `examples/stdlib.hsabrc` with:

1. Clear comment explaining what it shows
2. Screenshot or example output
3. Any terminal requirements (Unicode, colors)
