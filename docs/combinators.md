# hsab Combinators Reference

Combinators are higher-order operations that transform, compose, or apply blocks and values. They are fundamental to hsab's expressive power, enabling functional programming patterns in a stack-based shell.

## Table of Contents

1. [Block Basics](#block-basics)
2. [Control Flow](#control-flow)
3. [List Operations](#list-operations)
4. [Higher-Order Combinators](#higher-order-combinators)
5. [Practical Examples](#practical-examples)

---

## Block Basics

### Block Syntax `[...]`

Blocks are hsab's core abstraction for deferred execution. A block is a sequence of expressions wrapped in square brackets that is pushed to the stack as a single value rather than being executed immediately.

```bash
# Block pushed to stack (not executed)
[hello world echo]

# Stack now contains: [hello world echo]
```

Blocks are first-class values:
- They can be stored in definitions
- Passed to other operations
- Duplicated with `dup`
- Applied with `@`

```bash
# Store a block as a named definition
[curl -s https://api.example.com/health] :healthcheck

# Use it later
healthcheck @                   # Execute the block
healthcheck dup @ swap @        # Execute twice
```

### Apply Operator `@`

The `@` operator pops a block from the stack and executes it.

```bash
# Stack: [hello echo]
@
# Executes: hello echo
# Output: hello
```

Apply is essential for deferred execution:

```bash
# Without @, the block just sits on the stack
[ls -la]                        # Pushes block
.s                              # Shows: [ls -la]

# With @, it executes
[ls -la] @                      # Runs ls -la
```

### Blocks as Quotations

In stack-based languages, blocks are called "quotations" because they quote (defer) their contents. This enables:

**Passing code to functions:**
```bash
3 [hello echo] times           # Repeat 3 times
```

**Conditional execution:**
```bash
[file.txt -f test] [exists echo] [missing echo] if
```

**Building pipelines:**
```bash
ls [grep .rs] |                 # Pipe through grep
```

**Storing reusable operations:**
```bash
[dup .bak suffix cp] :backup    # Define backup operation
file.txt backup                 # Use it
```

---

## Control Flow

### if: Conditional Execution

**Syntax:** `[condition] [then] [else] if`

Evaluates the condition block. If exit code is 0 (success), executes the then-block; otherwise executes the else-block.

```bash
# Basic conditional
[file.txt -f test] [found echo] [not-found echo] if

# Numeric comparison
[5 3 gt?] [bigger echo] [smaller echo] if

# String comparison
[hello hello eq?] [same echo] [different echo] if
```

**How conditions work:**
- Exit code 0 = condition true (success)
- Non-zero exit code = condition false (failure)
- Predicates like `file?`, `eq?`, `lt?` return exit codes

```bash
# File tests
[path -f test] [is-file] [not-file] if
[path -d test] [is-dir] [not-dir] if
[path -e test] [exists] [missing] if

# Comparisons (exit 0 if true)
[a b eq?] [equal] [not-equal] if
[3 5 lt?] [less] [not-less] if
```

### when / unless: Single-Branch Conditionals

**Note:** `when` and `unless` are typically defined in the stdlib, not as builtins. Check your `~/.hsab/lib/stdlib.hsabrc` or define them:

```bash
# Define when: execute block if condition passes
[[drop] if] :when

# Define unless: execute block if condition fails
[[swap drop] if] :unless

# Usage
[file.txt -f test] [processing... echo] when
[deps -d test] [npm install] unless
```

### times: Repeat N Times

**Syntax:** `N [block] times`

Executes the block N times.

```bash
# Basic repetition
3 [hello echo] times
# Output:
# hello
# hello
# hello

# With stack operations
5 [dup echo 1 plus] times       # Echo 5, 6, 7, 8, 9

# Building sequences
marker 5 [dup 1 plus] times collect
# Stack: 1 2 3 4 5 (as newline-separated output)
```

### while: Loop While Condition Passes

**Syntax:** `[condition] [body] while`

Executes body repeatedly while condition returns exit code 0.

```bash
# Count down from 5
5
[dup 0 gt?] [dup echo 1 minus] while
# Output: 5, 4, 3, 2, 1

# Process until empty
[depth 0 gt?] [pop-and-process] while
```

**Important:** The condition is evaluated fresh each iteration. Any values pushed during condition evaluation are cleaned up.

### until: Loop Until Condition Passes

**Syntax:** `[condition] [body] until`

Opposite of while - executes body repeatedly until condition returns exit code 0.

```bash
# Keep trying until success
[curl -s $url] [1 sleep] until

# Read until specific input
["" $input eq?] [read-input] until
```

### break: Exit Loop Early

Immediately exits the enclosing `times`, `while`, or `until` loop.

```bash
# Find first match
10 [
  dup check-condition
  [break] [1 plus] if
] times

# Early exit on error
[true] [
  process-item
  [error?] [break] [] if
] while
```

---

## List Operations

List operations work with sequences of values on the stack. The `marker` establishes a boundary, and other operations work with items above it.

### marker: Push Stack Boundary

Pushes a special marker value to the stack that serves as a boundary for list operations.

```bash
marker                          # Stack: |marker|
1 2 3                          # Stack: |marker| 1 2 3
collect                        # Stack: "1\n2\n3"
```

### spread: Split Value onto Stack

Splits a value into separate stack items, pushing a marker first.

**For strings:** Splits by newlines
```bash
"a\nb\nc" spread
# Stack: |marker| "a" "b" "c"
```

**For lists:** Pushes each item
```bash
'[1, 2, 3]' json spread
# Stack: |marker| 1 2 3
```

**For maps:** Pushes each value
```bash
'{"a":1, "b":2}' json spread
# Stack: |marker| 1 2 (order undefined)
```

**Common pattern - spread command output:**
```bash
ls spread                       # File names on stack
# Stack: |marker| "file1" "file2" ...
```

### each: Apply Block to Each Item

**Syntax:** `spread [block] each`

Applies block to each item above the marker.

```bash
# Transform each file
ls spread [wc -l] each
# Each filename replaced with its line count

# Process each line
"a\nb\nc" spread [upper] each
# Stack: |marker| "A" "B" "C"
```

**Note:** `each` consumes the marker. Results remain on the stack.

### keep: Filter Items

**Syntax:** `spread [predicate] keep`

Keeps only items where predicate returns exit code 0.

```bash
# Keep only .rs files
*.* ls spread [".rs" ends?] keep
# Stack: |marker| (only .rs files)

# Keep numbers greater than 5
marker 1 2 7 3 9 [5 gt?] keep
# Stack: |marker| 7 9
```

### collect: Gather Back to Value

Gathers all items above the marker into a single newline-separated string.

```bash
marker 1 2 3 collect
# Stack: "1\n2\n3"

ls spread [".rs" ends?] keep collect
# Stack: "file1.rs\nfile2.rs\n..."
```

### map: Transform and Collect

**Syntax:** `spread [block] map`

Equivalent to `each` followed by `collect`. Applies block to each item and gathers results.

```bash
ls spread [wc -l] map
# Stack: "42\n17\n..." (line counts as single string)

# Transform and collect numbers
'[1,2,3]' json spread [2 mul] map
# Stack: "2\n4\n6"
```

### filter: Filter and Collect

**Syntax:** `spread [predicate] filter`

Equivalent to `keep` followed by `collect`. Filters items and gathers results.

```bash
ls spread [-f test] filter
# Stack: "file1\nfile2\n..." (only regular files)

# Filter numbers
'[1,2,3,4,5]' json spread [2 gt?] filter
# Stack: "3\n4\n5"
```

---

## Higher-Order Combinators

### dip: Run Block on Second Item

**Syntax:** `a b [block] dip`

Temporarily removes the top item, executes block on the remaining stack, then restores the top item.

```bash
# Stack: 1 2
[3 plus] dip
# Result: 4 2 (1+3=4, then 2 restored on top)

# Useful for operating "under" the top
x y [process] dip
# Processes x, leaves y on top
```

**Use case:** When you need to operate on values below the top without disturbing it.

```bash
# Double the second value, keep top unchanged
5 10 [2 mul] dip
# Stack: 10 10 (5*2=10, original 10 restored)
```

### tap: Inspect Without Consuming

**Syntax:** `value [block] tap`

Executes block with a copy of the value, then restores the original value. Block's output is discarded.

```bash
# Debug: inspect value without changing it
some-value [.s] tap           # Shows stack, value unchanged

# Log while passing through
data [dup "Processing: " swap format echo] tap process
# Logs, then continues with original data
```

**Difference from dip:** `tap` operates on the value itself (copying it), while `dip` operates on the stack below the value.

### fanout: Apply Multiple Blocks to Same Value

**Syntax:** `value [block1] [block2] [block3] fanout`

Runs the input value through each block, collecting all results.

```bash
# Apply multiple operations
"hello" [len] [upper] fanout
# Stack: 5 "HELLO"

# Test same input multiple ways
url [curl -I] [curl -s] [ping] fanout
# Stack: headers content ping-result

# Extract multiple fields
data [name get] [age get] [email get] fanout
# Stack: name-val age-val email-val
```

**How it works:**
1. Pops all blocks from stack (until non-block value)
2. Pops input value
3. Runs input through each block in order
4. Pushes all results to stack

### compose: Create Combined Block

**Syntax:** `[block1] [block2] compose` or `list-of-blocks compose`

Combines multiple blocks into a single pipeline block.

```bash
# Compose two operations
[len] [2 mul] compose
# Creates: [len 2 mul]

# Compose from list
marker [upper] [reverse] ["!" suffix] collect compose :transform
"hello" transform
# Stack: "!OLLEH"

# Build dynamic pipelines
[parse] [validate] [format] compose :pipeline
data pipeline
```

**Use case:** Building reusable transformation pipelines dynamically.

### zip: Pair Two Lists

**Syntax:** `list1 list2 zip`

Pairs elements from two lists element-wise. Stops at the shorter list.

```bash
'["a","b","c"]' json '[1,2,3]' json zip
# Result: [[a,1], [b,2], [c,3]]

# Batch rename with zip
old-names new-names zip [[get 0] [get 1] bi mv] each

# Parallel arrays to records
keys values zip [record] each
```

### cross: Cartesian Product

**Syntax:** `list1 list2 cross`

Creates all combinations of elements from two lists (Cartesian product).

```bash
'["x","y"]' json '[1,2]' json cross
# Result: [[x,1], [x,2], [y,1], [y,2]]

# Test all combinations
hosts ports cross [[get 0] [get 1] bi connect-test] each

# Parameter sweep
params1 params2 cross [unpack run-experiment] each
```

### retry: Retry Until Success

**Syntax:** `N [block] retry`

Retries block up to N times until it succeeds (exit code 0).

```bash
# Retry network operation
3 [curl -sf $url] retry

# With delay between attempts (use retry-delay)
[curl -sf $url] 5 500 retry-delay    # 5 attempts, 500ms delay
```

**Behavior:**
- Succeeds immediately if block returns exit code 0
- Waits 100ms between retries (default)
- Returns error after N failures
- Last result stays on stack

### retry-delay: Retry with Custom Delay

**Syntax:** `[block] N delay_ms retry-delay`

Like retry, but with configurable delay between attempts.

```bash
# 5 retries, 1 second apart
[flaky-api] 5 1000 retry-delay

# Exponential backoff (manual)
1 [
  dup [operation] retry
  [break] [2 mul] if
] 5 times
```

---

## Practical Examples

### File Batch Rename

Rename all `.txt` files to `.md`:

```bash
*.txt ls spread
[dup .md reext] each          # old new old new pairs
.s                             # Preview
[mv] each                      # Execute renames
```

With collect for single command:

```bash
*.txt ls spread [dup .md reext mv] each
```

### Parallel Health Checks

Check multiple servers concurrently:

```bash
[
  [api.example.com ping]
  [db.example.com ping]
  [cache.example.com ping]
] parallel
# All results on stack
```

### Extract Multiple Fields

From JSON records:

```bash
# Using fanout to extract multiple fields at once
'{"name":"Alice","age":30,"city":"NYC"}' json
["name" get] ["age" get] ["city" get] fanout
# Stack: "Alice" 30 "NYC"
```

### Build Dynamic Transformations

```bash
# Collect transformations based on conditions
marker
[need-upper?] [[upper]] [] if
[need-trim?] [[trim]] [] if
[need-prefix?] [["pre-" swap suffix]] [] if
collect compose :my-transform

data my-transform
```

### Process Configuration Matrix

Test all combinations of configs:

```bash
'["debug","release"]' json '["x86","arm"]' json cross
[
  spread-head                   # mode rest
  swap spread-head              # mode arch rest
  drop                          # mode arch
  build-for                     # Custom function
] each
```

### Resilient API Calls

```bash
# Retry with exponential backoff
[fetch-api] :call
3 call retry
# Or with custom delay
call 5 2000 retry-delay
```

### Pipeline Composition

Build reusable data pipelines:

```bash
# Define stages
[json] :parse
["items" get spread] :extract
[[valid?] keep] :filter
[to-csv] :format

# Compose into pipeline
[parse] [extract] [filter] [format] compose :process

# Use on any input
cat data.json [process] |
```

### Stream Processing

Process logs with dip and tap:

```bash
# Process while keeping original for logging
log-entry
[parse-json] tap        # Inspect parsed, keep original
["errors" get] dip      # Process, keep original on top
[archive] @             # Archive original
```

---

## Summary Table

| Combinator | Syntax | Effect |
|------------|--------|--------|
| `@` | `[block] @` | Execute block |
| `if` | `[cond] [then] [else] if` | Conditional branch |
| `times` | `N [block] times` | Repeat N times |
| `while` | `[cond] [body] while` | Loop while true |
| `until` | `[cond] [body] until` | Loop until true |
| `break` | `break` | Exit current loop |
| `marker` | `marker` | Push stack boundary |
| `spread` | `value spread` | Split onto stack |
| `each` | `spread [block] each` | Apply to each |
| `keep` | `spread [pred] keep` | Filter items |
| `collect` | `...items... collect` | Gather to value |
| `map` | `spread [block] map` | each + collect |
| `filter` | `spread [pred] filter` | keep + collect |
| `dip` | `a b [block] dip` | Run block on second |
| `tap` | `value [block] tap` | Inspect, preserve value |
| `fanout` | `value [f] [g] fanout` | Apply multiple blocks |
| `compose` | `[f] [g] compose` | Combine blocks |
| `zip` | `list1 list2 zip` | Pair elements |
| `cross` | `list1 list2 cross` | Cartesian product |
| `retry` | `N [block] retry` | Retry until success |
| `retry-delay` | `[block] N ms retry-delay` | Retry with delay |

---

See also:
- `hsab --help` for complete builtin reference
- [README.md](../README.md) for shell overview
- [extending-stdlib.md](extending-stdlib.md) for custom definitions
