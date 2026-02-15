# Configuration and Customization

This document covers how to configure and customize hsab, including configuration files, environment variables, prompt customization, and REPL commands.

## Configuration Files

hsab uses several configuration files, loaded in a specific order depending on how the shell is invoked.

### ~/.hsabrc

The primary user configuration file. Executed every time an interactive REPL session starts.

```bash
# Example ~/.hsabrc
"Welcome to hsab!" println

# Define custom aliases
{ 2 * } 'double' def
{ dup * } 'square' def

# Load frequently used modules
"math" require
```

### ~/.hsab_profile

Executed only for login shells (when hsab is invoked with the `-l` flag). Use this for environment setup that should only happen once per login session.

```bash
# Example ~/.hsab_profile
# Set up paths
"/usr/local/lib/hsab" "HSAB_PATH" setenv

# Display login message
"Login shell initialized" println
```

### ~/.hsab/lib/stdlib.hsabrc

The standard library location. If present, this file is loaded before user configuration. This is where you can place commonly shared functions and definitions.

```
~/.hsab/
└── lib/
    └── stdlib.hsabrc
```

### Loading Order

Configuration files are loaded in the following order:

1. **Built-in defaults** - Internal default settings
2. **~/.hsab/lib/stdlib.hsabrc** - Standard library (if exists)
3. **~/.hsab_profile** - Login profile (only with `-l` flag)
4. **~/.hsabrc** - User configuration (interactive sessions)

Each subsequent file can override settings from previous files.

## Environment Variables

hsab behavior can be controlled through environment variables. These can be set in your shell's profile (e.g., `~/.bashrc`, `~/.zshrc`) or within hsab configuration files using `setenv`.

### HSAB_PATH

Module search path for `require` statements. Multiple paths are separated by colons.

```bash
export HSAB_PATH="/usr/local/lib/hsab:$HOME/.hsab/modules"
```

When you call `"mymodule" require`, hsab searches these directories in order.

### HSAB_MAX_RECURSION

Maximum recursion depth to prevent stack overflow. Default is typically 1000.

```bash
export HSAB_MAX_RECURSION=2000
```

### HSAB_BANNER

Controls whether the startup banner is displayed. Set to `0` or `false` to disable.

```bash
export HSAB_BANNER=0
```

### HSAB_UNDO_DEPTH

Number of undo states to keep in history. Higher values use more memory.

```bash
export HSAB_UNDO_DEPTH=100
```

### HSAB_PREVIEW_LEN

Maximum length of the limbo (pending input) preview shown in the prompt.

```bash
export HSAB_PREVIEW_LEN=40
```

### HSAB_THREAD_POOL_SIZE

Number of threads in the async execution pool for concurrent operations.

```bash
export HSAB_THREAD_POOL_SIZE=8
```

### HSAB_HIGHLIGHT

Enable or disable syntax highlighting. Set to `1` or `true` to enable.

```bash
export HSAB_HIGHLIGHT=1
```

### HSAB_SUGGESTIONS

Enable fish-style autosuggestions based on history. Set to `1` or `true` to enable.

```bash
export HSAB_SUGGESTIONS=1
```

### HSAB_SUGGESTION_ARROW

Character(s) used to indicate suggestions. Default is typically `→` or `->`.

```bash
export HSAB_SUGGESTION_ARROW="→"
```

## Prompt Customization

hsab provides customizable prompts through special variables.

### _PS1 - Main Prompt

The primary prompt displayed when waiting for input.

```bash
# Simple prompt
"hsab> " '_PS1' def

# Prompt with stack depth
{ depth str ":" + "hsab> " + } '_PS1' def
```

### _PS2 - Continuation Prompt

Displayed when input continues across multiple lines (e.g., unclosed braces).

```bash
"... " '_PS2' def
```

### _LIMBO

Variable containing the count of items in limbo (pending evaluation).

```bash
# Show limbo count in prompt
{ _LIMBO str " items | hsab> " + } '_PS1' def
```

### _FUTURES

Variable containing the count of pending async futures.

```bash
# Show futures in prompt
{ _FUTURES 0 > { "[" _FUTURES str + " async] " + } when } '_PS1' def
```

### Prompt Escapes and Colors

Use ANSI escape codes for colored prompts:

```bash
# Red prompt
"\x1b[31mhsab>\x1b[0m " '_PS1' def

# Green with bold
"\x1b[1;32mhsab>\x1b[0m " '_PS1' def

# Blue stack depth, white text
{ "\x1b[34m[" depth str + "]\x1b[0m hsab> " + } '_PS1' def
```

Common color codes:
- `\x1b[30m` - Black
- `\x1b[31m` - Red
- `\x1b[32m` - Green
- `\x1b[33m` - Yellow
- `\x1b[34m` - Blue
- `\x1b[35m` - Magenta
- `\x1b[36m` - Cyan
- `\x1b[37m` - White
- `\x1b[0m` - Reset
- `\x1b[1m` - Bold

## REPL Commands

Special dot-commands provide runtime control of the REPL environment.

### .debug / .d

Toggle debug mode to see detailed execution information.

```
hsab> .debug
Debug mode: ON

hsab> 2 3 +
[DEBUG] Push: 2
[DEBUG] Push: 3
[DEBUG] Apply: +
[DEBUG] Result: 5
5

hsab> .d
Debug mode: OFF
```

### .step

Enable step mode for line-by-line execution with pauses.

```
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

### .highlight on/off

Toggle syntax highlighting at runtime.

```
hsab> .highlight on
Syntax highlighting: ON

hsab> .highlight off
Syntax highlighting: OFF
```

### .suggestions on/off

Toggle fish-style autosuggestions.

```
hsab> .suggestions on
Suggestions: ON

hsab> .suggestions off
Suggestions: OFF
```

### .stack / .s

Display the current stack contents.

```
hsab> 1 2 3
hsab> .stack
Stack (3 items):
  [0] 1
  [1] 2
  [2] 3

hsab> .s
[1, 2, 3]
```

### .clear

Clear the terminal screen.

```
hsab> .clear
```

### .help

Display help information about available commands and operations.

```
hsab> .help
hsab - a postfix notation shell

REPL Commands:
  .debug, .d     Toggle debug mode
  .step          Toggle step mode
  .highlight     Toggle syntax highlighting
  .suggestions   Toggle autosuggestions
  .stack, .s     Show stack contents
  .clear         Clear screen
  .help          Show this help

Use 'words' to list defined words.
Use '"topic" help' for specific help.
```

## History

hsab maintains command history across sessions.

### ~/.hsab_history

Command history is stored in `~/.hsab_history`. This file is updated when the REPL exits.

### History Size

The number of history entries kept can be configured:

```bash
export HSAB_HISTORY_SIZE=10000
```

### Ctrl+R Search

Press `Ctrl+R` to enter reverse incremental search mode:

```
hsab> ^R
(reverse-i-search)`def': { dup * } 'square' def
```

Type characters to search backward through history. Press:
- `Ctrl+R` again to find the next match
- `Enter` to execute the found command
- `Ctrl+G` or `Escape` to cancel search
- `Ctrl+J` to edit the command before executing

Additional history navigation:
- `Up Arrow` / `Ctrl+P` - Previous command
- `Down Arrow` / `Ctrl+N` - Next command
- `Ctrl+A` - Move to beginning of line
- `Ctrl+E` - Move to end of line

## Example .hsabrc

Here is a comprehensive example configuration file:

```bash
# ~/.hsabrc - hsab configuration

# ============================================
# Prompt Customization
# ============================================

# Colorful prompt showing stack depth and limbo count
{
  "\x1b[36m[\x1b[0m"          # Cyan bracket
  depth str +                  # Stack depth
  "\x1b[36m|\x1b[0m" +        # Cyan separator
  _LIMBO str +                 # Limbo count
  "\x1b[36m]\x1b[0m " +       # Cyan bracket
  "\x1b[1;32mhsab>\x1b[0m " + # Bold green prompt
} '_PS1' def

# Continuation prompt
"\x1b[33m...\x1b[0m " '_PS2' def

# ============================================
# Custom Aliases and Functions
# ============================================

# Math helpers
{ 2 * } 'double' def
{ 2 / } 'half' def
{ dup * } 'square' def
{ dup dup * * } 'cube' def

# Stack manipulation shortcuts
{ swap drop } 'nip' def
{ swap over } 'tuck' def
{ dup rot rot } 'dup-under' def

# List utilities
{ [] swap { swap cons } fold } 'reverse' def
{ length 0 = } 'empty?' def

# String utilities
{ " " split } 'words' def
{ "\n" split } 'lines' def
{ "" join } 'concat-all' def

# Debugging helpers
{ .stack } 'ss' def
{ depth println } 'sd' def

# ============================================
# Load Modules
# ============================================

# Load standard modules (if they exist)
# "math" require
# "string" require
# "list" require

# ============================================
# Default Settings
# ============================================

# Enable syntax highlighting
# .highlight on

# Enable suggestions
# .suggestions on

# ============================================
# Startup Actions
# ============================================

# Display welcome message (optional)
# "\x1b[1;34m" "Welcome to hsab!" + "\x1b[0m" + println
# "Type .help for assistance" println
# "" println

# Show current date/time
# "now" require
# "Started at: " now str + println
```

### Minimal .hsabrc

For a minimal configuration:

```bash
# ~/.hsabrc - minimal config

# Simple prompt
"hsab> " '_PS1' def

# Essential aliases
{ dup * } 'sq' def
{ 2 * } 'dbl' def
```

### Advanced .hsabrc with Conditionals

```bash
# ~/.hsabrc - advanced config

# Platform-specific settings
"uname" shell "Darwin" =
{
  # macOS specific
  "/opt/homebrew/lib/hsab" "HSAB_PATH" setenv
}
{
  # Linux/other
  "/usr/local/lib/hsab" "HSAB_PATH" setenv
} if

# Check for interactive mode before loading heavy modules
# (useful if .hsabrc is sourced in non-interactive contexts)

# Define a project-specific loader
{
  "./.hsabrc.local" file-exists?
  { "./.hsabrc.local" load } when
} 'load-local-config' def

# Automatically load local config if present
load-local-config
```

## See Also

- [Getting Started](./getting-started.md) - Introduction to hsab
- [Language Reference](./language.md) - Complete language documentation
- [Modules](./modules.md) - Module system documentation
