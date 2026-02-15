# Definitions, Locals, and Modules

This guide covers how to define reusable words, manage local variables, and organize code into modules in hsab.

---

## Defining Words

In hsab, a **definition** binds a block to a name. Definitions are the primary way to create reusable functions.

### Syntax

```bash
[body] :name
```

The `:name` syntax pops the block from the stack and stores it as a named word. You can then invoke it by name:

```bash
[dup *] :square    # Define "square"
5 square           # Invoke it: pushes 25
```

### Definitions Are Just Stored Blocks

Under the hood, definitions are blocks stored in a lookup table. When you invoke a defined word, hsab finds the stored block and executes it. This means:

- Definitions can call other definitions
- Definitions can be redefined at any time
- Recursion works naturally (with a depth limit of 10000 by default)

```bash
# Factorial using recursion
[
  dup 1 le?
    [drop 1]
    [dup 1 minus factorial *]
  if
] :factorial

5 factorial    # 120
```

### Simple Examples

```bash
# Square a number
[dup *] :square
5 square           # 25

# Convert to backup filename
[.bak suffix] :backup-name
"file.txt" backup-name    # "file.txt.bak"

# Check if file is a Rust source
[".rs" ends?] :rust-file?
"main.rs" rust-file?      # true

# Git status shortcut
[status git] :gs
gs                        # Shows git status
```

### Multi-Line Definitions

Blocks can span multiple lines. The shell accumulates input until brackets are balanced:

```bash
[
  dup 0 lt?
    [drop 0]           # If negative, replace with 0
    []                 # Otherwise keep as-is
  if
] :clamp-positive
```

---

## Local Variables

Inside definitions, `local` creates scoped variables that are automatically restored when the function exits.

### Syntax

```bash
value NAME local     # Create local variable NAME with value
$NAME                # Access it like any env var
```

### Basic Usage

```bash
[
  _X local           # Pop value into _X
  $_X $_X *          # Use it (square)
] :square-local

5 square-local       # 25
```

**Convention:** Local variable names often start with underscore (`_X`, `_NAME`) to distinguish them from global environment variables.

### Structured Data Preservation

Local variables handle different types differently:

- **Primitives** (strings, numbers, booleans): Stored as environment variables for shell compatibility
- **Structured data** (Lists, Tables, Maps, BigInt, Bytes, Media, Blocks): Stored internally to preserve their type

```bash
# Primitive: uses env var
[
  42 _NUM local
  $_NUM 8 plus       # 50
] :add-eight

# Structured: preserves List type
[
  '[1,2,3,4,5]' into-json _NUMS local
  $_NUMS sum         # 15 - List operations work!
] :sum-list

sum-list
```

Without type preservation, the list would be stringified and `sum` would fail.

### Scope and Cleanup

Local variables exist only within their function scope. When the function exits:

1. Variables that existed before are restored to their original values
2. Variables that didn't exist are unset

```bash
# Original value preserved
X=original
[inner _X local $_X echo] :test
test                 # Prints "inner"
$X echo              # Prints "original" - restored!

# New variable cleaned up
[newval _TEMP local $_TEMP] :temp-test
temp-test            # Prints "newval"
$_TEMP echo          # Empty - unset after function
```

---

## Nested Scopes

Each function call creates its own scope. Inner scopes can shadow outer variables.

### Shadowing Example

```bash
[
  100 _VAL local
  $_VAL               # Returns 100
] :inner

[
  5 _VAL local
  inner               # Inner shadows with 100
  $_VAL               # Outer's _VAL is still 5
  plus
] :outer

outer                 # 105
```

### Deep Nesting

```bash
[1000 _LEVEL local $_LEVEL] :level3
[100 _LEVEL local level3 $_LEVEL plus] :level2
[10 _LEVEL local level2 $_LEVEL plus] :level1

level1               # 1110 (1000 + 100 + 10)
```

Each level has its own independent `_LEVEL` variable.

### Error: Local Outside Function

`local` only works inside function calls:

```bash
42 _VAR local
# Error: local: can only be used inside a function
```

This is intentional - at the top level, use environment variables or assignments.

---

## Module System

Modules let you organize definitions into separate files and import them with namespacing.

### Importing Modules

```bash
"path/to/module.hsab" .import
```

The `.import` command:
1. Reads and executes the module file
2. Namespaces all definitions with the module name
3. Prevents double-loading (importing the same module twice is a no-op)

```bash
# In myutils.hsab:
[dup .bak suffix] :backup

# In your script:
"myutils.hsab" .import
"file.txt" myutils::backup    # "file.txt.bak"
```

### Import with Alias

You can specify a custom namespace:

```bash
"path/to/long-module-name.hsab" utils .import
"file.txt" utils::backup      # Uses "utils" instead of "long-module-name"
```

### Private Definitions

Definitions starting with underscore (`_`) are private and not exported:

```bash
# In mymodule.hsab:
[internal helper] :_helper    # Private
[_helper do-something] :public

# After import:
mymodule::public              # Works
mymodule::_helper             # Treated as literal (not found)
```

### Module Search Path

When importing, hsab searches for modules in this order:

1. Current directory (`.`)
2. `./lib/`
3. `~/.hsab/lib/`
4. Directories in `$HSAB_PATH` (colon-separated)

```bash
# These all work if the file exists in the search path:
"utils.hsab" .import
"mylib/helpers.hsab" .import
"/absolute/path/module.hsab" .import
```

### Circular Import Protection

Modules track their canonical path. If a module has already been loaded, importing it again is a no-op. This prevents infinite loops with circular dependencies.

---

## Standard Library

hsab includes a standard library with common definitions.

### Location

```
~/.hsab/lib/stdlib.hsabrc
```

### Installation

```bash
hsab init
```

This copies the embedded stdlib to `~/.hsab/lib/stdlib.hsabrc`. The stdlib is loaded automatically when hsab starts.

### Common Definitions

The stdlib provides shortcuts for common operations:

```bash
# Navigation
ll                   # ls -la
la                   # ls -lah
l1                   # ls -1

# Git
gs                   # git status
gd                   # git diff
gl                   # git log --oneline -20

# Arithmetic
5 inc                # 6
7 dec                # 6
-5 abs               # 5
3 7 min              # 3
3 7 max              # 7

# String predicates
"hello" "he" starts?     # true
"hello" "lo" ends?       # true
"hello" "ll" contains?   # true

# Statistics (on lists)
'[1,2,3,4,5]' into-json median     # 3
'[1,2,3,4,5]' into-json variance   # 2
'[1,2,3,4,5]' into-json std-dev    # 1.414...
```

See `~/.hsab/lib/stdlib.hsabrc` for the full list.

---

## Configuration Files

hsab loads configuration files in a specific order.

### ~/.hsabrc

Runs every time the REPL starts. Use for:

- Custom definitions
- Aliases
- Environment setup
- Prompt customization

```bash
# ~/.hsabrc

# Custom definitions
[-la ls] :ll
[status git] :gs

# Environment
PATH=$PATH:~/bin .export

# Custom prompt
[
  "hsab-" $_VERSION suffix
  [$_DEPTH 0 gt?] ["$ "] ["> "] if suffix
] :PS1
```

### ~/.hsab_profile

Runs only for login shells. Use for:

- One-time session setup
- Environment variables needed by child processes

```bash
# ~/.hsab_profile

EDITOR=vim .export
LANG=en_US.UTF-8 .export
```

### Loading Order

1. **stdlib** (`~/.hsab/lib/stdlib.hsabrc`) - if installed
2. **profile** (`~/.hsab_profile`) - login shells only
3. **rc** (`~/.hsabrc`) - always

Later files can override earlier definitions.

---

## Best Practices

### Naming Conventions

| Type | Convention | Example |
|------|------------|---------|
| Local variables | Underscore prefix | `_X`, `_TEMP`, `_DATA` |
| Private definitions | Underscore prefix | `:_helper` |
| Predicates | Question mark suffix | `:empty?`, `:valid?` |
| Destructive ops | Exclamation suffix | `:clear!` |
| Abbreviations | Lowercase, short | `:gs`, `:ll`, `:cb` |

### Documentation in Comments

```bash
# backup-file: Create a .bak copy of a file
# Stack: filename --
# Example: "data.txt" backup-file
[dup .bak suffix cp] :backup-file
```

### Organizing Definitions

**Small projects:** Put everything in `~/.hsabrc`

**Larger projects:** Create modules in `~/.hsab/lib/`:

```
~/.hsab/lib/
  stdlib.hsabrc      # Built-in standard library
  git.hsab           # Git shortcuts
  docker.hsab        # Docker utilities
  myproject.hsab     # Project-specific
```

Then import what you need:

```bash
# In ~/.hsabrc
"git.hsab" .import
"docker.hsab" .import
```

### Keep Definitions Small

Prefer many small definitions over few large ones:

```bash
# Good: composable pieces
[".rs" ends?] :rust?
[".py" ends?] :python?
[-1 ls spread [rust?] keep] :rust-files
[-1 ls spread [python?] keep] :python-files

# Less good: monolithic
[
  "-1" ls spread
  [dup ".rs" ends? swap ".py" ends? or] keep
] :code-files
```

### Use Local for Clarity

When a value is used multiple times, `local` makes intent clear:

```bash
# Without local: cryptic stack juggling
[dup dup * swap 2 * plus] :some-formula

# With local: readable
[
  _X local
  $_X $_X *          # x^2
  $_X 2 * plus       # x^2 + 2x
] :some-formula
```

---

## Quick Reference

| Operation | Syntax | Description |
|-----------|--------|-------------|
| Define | `[body] :name` | Store block as named word |
| Invoke | `name` | Execute stored block |
| Local | `value NAME local` | Create scoped variable |
| Access local | `$NAME` | Expand variable value |
| Import | `"path.hsab" .import` | Load module |
| Import with alias | `"path.hsab" alias .import` | Load with custom namespace |
| Call namespaced | `module::func` | Call imported function |
