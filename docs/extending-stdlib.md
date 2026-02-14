# Extending the hsab Standard Library

hsab has two configuration files:

| File | Purpose | Loaded |
|------|---------|--------|
| `~/.hsab/lib/stdlib.hsabrc` | Standard library (shared definitions) | First |
| `~/.hsabrc` | Your personal config | Second |

The stdlib is installed via `hsab init` and provides community-contributed definitions. Your `~/.hsabrc` is for personal customizations.

## Quick Start

### Add to Your Personal Config

```bash
vim ~/.hsabrc
# or
code ~/.hsabrc
# or from within hsab:
~/.hsabrc vim
```

Add a definition:

```bash
# My custom shortcut
[-la ls] :ll
```

Reload without restarting:

```bash
~/.hsabrc .source
```

### Modify the Standard Library

To customize or extend the stdlib itself:

```bash
vim ~/.hsab/lib/stdlib.hsabrc
```

Reload:

```bash
~/.hsab/lib/stdlib.hsabrc .source
```

**Tip:** Keep personal shortcuts in `~/.hsabrc` and only modify stdlib for fixes or contributions you plan to share.

## Anatomy of a Definition

```bash
[BLOCK] :NAME
```

- **BLOCK**: A sequence of operations enclosed in `[ ]`
- **NAME**: The word that will invoke this block (prefixed with `:`)

The block captures operations to run later. When you type `NAME`, hsab pushes the block's operations onto the execution queue.

## Writing Effective Definitions

### 1. Simple Aliases

Wrap common flag combinations:

```bash
[-la ls] :ll                    # ll → ls -la
[status git] :gs                # gs → git status
[--oneline -20 log git] :gl     # gl → git log --oneline -20
```

### 2. Stack-Native Functions

hsab definitions naturally consume from the stack:

```bash
# file.txt wc-l → line count of file.txt
[-l wc] :wc-l

# /path/to file.txt path-join → /path/to/file.txt
# (path-join is already a builtin, but shows the pattern)
```

### 3. Using Local Variables

For complex logic, use `local` to name intermediate values:

```bash
# Absolute value: -5 abs → 5
[_N local [$_N 0 lt?] [0 $_N minus] [$_N] if] :abs

# Min of two numbers: 3 7 min → 3
[_B local _A local [$_A $_B lt?] [$_A] [$_B] if] :min
```

**Key points:**
- `local` pops a name, then pops a value
- Prefix with `_` by convention (avoids conflicts)
- Access with `$_NAME`
- Structured data (Lists, Tables, Maps) preserves type
- Primitives use env vars (shell compatible)

### 4. Working with Lists

```bash
# Sum a list: '[1,2,3,4,5]' into-json my-sum → 15
[sum] :my-sum

# Double each element
[[2 mul] map] :double-all

# Filter positive numbers
[[0 gt?] filter] :positives
```

### 5. Conditionals

```bash
# Check file type and act accordingly
[
  dup file?
  [cat]
  [dup dir? ["Directory:" swap suffix echo] ["Unknown:" swap suffix echo] if]
  if
] :show
```

### 6. Loops

```bash
# Print numbers 1 to N: 5 count-to → prints 1 2 3 4 5
[
  _N local
  1 _I local
  [$_I $_N le?] [
    $_I echo
    $_I 1 plus _I local
  ] while
] :count-to
```

## Best Practices

### Naming Conventions

| Pattern | Use Case | Example |
|---------|----------|---------|
| `verb` | Actions | `backup`, `deploy` |
| `noun-verb` | Specific actions | `git-sync`, `file-clean` |
| `?` suffix | Predicates | `empty?`, `valid?` |
| Short (2-3 chars) | Frequent commands | `gs`, `ll`, `gd` |

### Documentation

Add comments above your definitions:

```bash
# ============================================
# MY PROJECT SHORTCUTS
# ============================================

# Deploy to staging server
# Usage: deploy-staging
[
  build cargo
  "Deploying..." echo
  # ... deployment logic
] :deploy-staging
```

### Modularity

Group related definitions:

```bash
# === GIT ===
[status git] :gs
[diff git] :gd
[--cached diff git] :gdc

# === DOCKER ===
[ps docker] :dps
[images docker] :di
```

### Testing Your Definitions

Test interactively before adding to `~/.hsabrc`:

```bash
hsab
£ [-la ls] :ll
£ ll
# verify it works
£ # then add to ~/.hsabrc
```

## Advanced Patterns

### Composition

Chain definitions together:

```bash
[sort-nums dup count 2 idiv nth] :median
[variance sqrt] :std-dev
```

### Error Handling

Use `try` for operations that might fail:

```bash
[
  [risky-operation] try
  error? ["Failed!" echo] [echo] if
] :safe-op
```

### Working with External Commands

Definitions can wrap any shell command:

```bash
# Fuzzy find and edit
[fzf | vim] :fv

# Search and replace in files
[_TO local _FROM local
 . -type f find spread
 [$_FROM $_TO sed -i] each
] :replace-in-files
```

## Example: Complete Workflow

Here's a real-world example adding project-specific shortcuts:

```bash
# ============================================
# RUST PROJECT WORKFLOW
# ============================================

# Quick build and test cycle
[build cargo && test cargo] :ct

# Release build with size check
[
  --release build cargo
  "Binary size:" echo
  target/release/* -type f find spread
  [du -h] each
] :release

# Format, lint, and test (pre-commit)
[
  fmt cargo
  clippy cargo
  test cargo
  "All checks passed!" echo
] :precommit
```

## Sharing Your Definitions

The stdlib source is at `examples/stdlib.hsabrc` in the hsab repo. To contribute:

1. Test your definition thoroughly
2. Add clear comments explaining usage
3. Follow the section organization in the file
4. Submit a PR to the hsab repository

Your changes will be included in the next `hsab init` for all users.

## Troubleshooting

**Definition not found?**
- Check spelling
- Run `~/.hsabrc .source` to reload
- Verify syntax with `cat ~/.hsabrc | head`

**Stack underflow?**
- Your definition expects more arguments than provided
- Add `depth` checks or document required inputs

**Unexpected behavior?**
- Use `--trace` flag: `hsab --trace -c "your-def"`
- Break complex definitions into smaller pieces
