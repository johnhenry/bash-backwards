# Stack Operations in hsab

The stack is the central data structure in hsab. Understanding how it works is essential for effective use of the shell.

## Mental Model: LIFO

The stack operates as a **Last-In-First-Out (LIFO)** structure. Think of it like a stack of plates:

- You can only add plates to the top
- You can only remove plates from the top
- The last plate you put on is the first one you take off

```
       +-------+
       |   c   |  <- top (most recently added)
       +-------+
       |   b   |
       +-------+
       |   a   |  <- bottom (first added)
       +-------+
```

When you type values in hsab, they get pushed onto the stack. When operations run, they consume values from the top and push results back.

```bash
> 1 2 3
# Stack is now: 1 2 3 (3 is on top)

> plus
# Pops 3 and 2, pushes 5
# Stack is now: 1 5
```

### Visualizing Stack Flow

Read hsab code left-to-right. Each word either:
- **Pushes** a value onto the stack (literals, command outputs)
- **Pops** values, operates, and **pushes** results (operations)

```
1 2 plus 3 mul
|   |    |     |
v   v    |     |
[1] |    |     |
[1,2]    |     |
     [3] <-- plus pops 1,2 pushes 3
         [3,3]
             [9] <-- mul pops 3,3 pushes 9
```

---

## Core Stack Operations

These are hsab builtins for manipulating the stack directly.

### dup - Duplicate Top

Copies the top element without removing it.

```
Before:     After:
+---+       +---+
| a |       | a |  <- copy
+---+       +---+
| b |       | a |  <- original
+---+       +---+
            | b |
            +---+
```

```bash
> 5 dup
# Stack: 5 5

> "hello" dup
# Stack: "hello" "hello"
```

**Use case:** When you need to use a value twice.

```bash
> file.txt dup wc -l swap cat
# Count lines AND display contents
```

---

### drop - Discard Top

Removes the top element without returning it.

```
Before:     After:
+---+       +---+
| a |       | b |
+---+       +---+
| b |       | c |
+---+       +---+
| c |
+---+
```

```bash
> 1 2 3 drop
# Stack: 1 2

> "unwanted" "wanted" drop
# Stack: "unwanted"
```

**Use case:** Discard intermediate results you don't need.

---

### swap - Swap Top Two

Exchanges the positions of the top two elements.

```
Before:     After:
+---+       +---+
| a |       | b |
+---+       +---+
| b |       | a |
+---+       +---+
| c |       | c |
+---+       +---+
```

```bash
> 1 2 swap
# Stack: 2 1

> "src" "dest" swap
# Stack: "dest" "src"
```

**Use case:** Reorder arguments for commands that expect them in a different order.

```bash
> dest.txt src.txt swap cp
# Equivalent to: cp src.txt dest.txt
```

---

### over - Copy Second to Top

Copies the second element to the top without removing it.

```
Before:     After:
+---+       +---+
| a |       | b |  <- copy of second
+---+       +---+
| b |       | a |
+---+       +---+
| c |       | b |  <- original still there
+---+       +---+
            | c |
            +---+
```

```bash
> 1 2 over
# Stack: 1 2 1

> "a" "b" over
# Stack: "a" "b" "a"
```

**Use case:** Access the second element without losing the top.

```bash
> old.txt new.txt over cat swap cp
# Read old.txt, then copy old.txt to new.txt
```

---

### rot - Rotate Top Three

Moves the third element to the top, shifting the others down.

```
Before:     After:
+---+       +---+
| a |       | c |  <- was third
+---+       +---+
| b |       | a |  <- was first
+---+       +---+
| c |       | b |  <- was second
+---+       +---+
```

```bash
> 1 2 3 rot
# Stack: 2 3 1

> "a" "b" "c" rot
# Stack: "b" "c" "a"
```

**Use case:** Access the third element or reorder three values.

---

### nip - Drop Second (stdlib)

Removes the second element, keeping the top. Defined in stdlib as `swap drop`.

```
Before:     After:
+---+       +---+
| a |       | a |  <- kept
+---+       +---+
| b |       | c |
+---+       +---+
| c |
+---+
```

```bash
> 1 2 3 nip
# Stack: 1 3

> "keep" "discard" "top" nip
# Stack: "keep" "top"
```

**Definition:** `[swap drop] :nip`

---

### tuck - Copy Top Below Second (stdlib)

Copies the top element below the second. Defined in stdlib as `dup rot`.

```
Before:     After:
+---+       +---+
| a |       | a |  <- original top
+---+       +---+
| b |       | b |
+---+       +---+
            | a |  <- copy tucked below
            +---+
```

```bash
> 1 2 tuck
# Stack: 2 1 2

> "a" "b" tuck
# Stack: "b" "a" "b"
```

**Definition:** `[dup rot] :tuck`

---

### -rot - Reverse Rotate (stdlib)

Reverse of `rot`: moves the top element to third position. Defined as `rot rot`.

```
Before:     After:
+---+       +---+
| a |       | b |  <- was second
+---+       +---+
| b |       | c |  <- was third
+---+       +---+
| c |       | a |  <- was first
+---+       +---+
```

```bash
> 1 2 3 -rot
# Stack: 3 1 2
```

**Definition:** `[rot rot] :-rot`

---

## Pair Operations (stdlib)

Operations that work on pairs of elements.

### 2dup - Duplicate Top Pair

Duplicates the top two elements.

```
Before:     After:
+---+       +---+
| a |       | a |
+---+       +---+
| b |       | b |
+---+       +---+
            | a |
            +---+
            | b |
            +---+
```

```bash
> 1 2 2dup
# Stack: 1 2 1 2
```

**Note:** The stdlib definition `[dup dup] :2dup` actually triplicates one element. A proper 2dup would be `[over over] :2dup`.

---

### 2drop - Drop Top Pair

Drops the top two elements.

```
Before:     After:
+---+       +---+
| a |       | c |
+---+       +---+
| b |
+---+
| c |
+---+
```

```bash
> 1 2 3 4 2drop
# Stack: 1 2
```

**Definition:** `[drop drop] :2drop`

---

### 2swap - Swap Top Two Pairs

Exchanges the top two pairs.

```
Before:     After:
+---+       +---+
| a |       | c |
+---+       +---+
| b |       | d |
+---+       +---+
| c |       | a |
+---+       +---+
| d |       | b |
+---+       +---+
```

This is not in the default stdlib but can be defined as needed.

---

## Stack Inspection

### .s / .stack - Show Stack Contents

The `.s` command displays the current stack state:

```bash
> 1 2 3
> .s
Stack: [Literal("1"), Literal("2"), Literal("3")]
```

This shows the internal representation, useful for debugging.

### depth - Push Stack Size

Pushes the current number of items on the stack:

```bash
> 1 2 3 depth
# Stack: 1 2 3 3  (three items, so 3 is pushed)

> depth
# Stack: 0  (empty stack)
```

---

## Stack Hints in the REPL

When running interactively, hsab shows a **stack hint** below your input. This gives you real-time visibility into stack contents:

```bash
hsab-0.5.0£ 1 2 3
1 2 3
hsab-0.5.0¢ plus
1 2 3
```

The hint updates as you type and use keyboard shortcuts:

| Shortcut | Action |
|----------|--------|
| **Alt+h** | Toggle hint visibility |
| **Alt+t** | Toggle type annotations in hint |

With type annotations enabled (Alt+t):

```
1(num) hello(str) [block](blk)
```

### Prompt Indicators

- `£` - Stack is empty
- `¢` - Stack has items

### Customizing the Hint Format

The hint format is controlled by the `STACK_HINT` definition in your config:

```bash
# Default: space-separated
["\n" " " str-replace] :STACK_HINT

# Comma-separated
["\n" ", " str-replace] :STACK_HINT

# Bracketed
["\n" " " str-replace "[" swap "]" suffix suffix] :STACK_HINT
# Produces: [a b c]
```

---

## Common Patterns

### Pattern 1: dup Before Destructive Operations

Many operations consume (pop) their arguments. Use `dup` to preserve a value:

```bash
# BAD: file.txt is consumed
> file.txt cat

# GOOD: keep file.txt for further operations
> file.txt dup cat
# Stack still has: file.txt

# Use it again
> wc -l
```

Real example - process a file and keep its name:

```bash
> data.csv dup [head -5] | .s
# Stack: data.csv (output was printed)
```

### Pattern 2: swap for Argument Reordering

Commands often expect arguments in a specific order. Use `swap` to reorder:

```bash
# You have: destination source
# cp expects: source destination
> dest.txt src.txt swap cp
```

Common reordering scenarios:

```bash
# String replace: have "new old string", need "old new string"
> "new" "old" "hello old world" rot swap str-replace
# Result: "hello new world"
```

### Pattern 3: over Instead of dup swap

When you need the second item but also want to keep the top:

```bash
# Verbose way:
> a b dup rot
# Stack: a b a

# Better way:
> a b over
# Stack: a b a
```

`over` is more efficient and clearer in intent.

### Pattern 4: Working with Command Results

Commands push their output to the stack. Chain operations:

```bash
# Get filename, check if it exists, and read it
> ls -1 spread     # filenames on stack
> [exists?] keep    # filter to existing
> [cat] each        # read each one
```

### Pattern 5: Stack as Scratchpad

Use the stack to accumulate results during exploration:

```bash
> *.log ls spread           # all log files
> .s                        # inspect
> [".gz" ends?] keep        # filter compressed
> .s                        # check again
> drop drop                 # remove two I don't want
> [zcat] each               # decompress and read
```

---

## Stack Underflow Errors

A **stack underflow** occurs when an operation needs more values than are available:

```bash
> swap
Error: Stack underflow in 'swap' (needs 2 items)

> 1 plus
Error: Stack underflow in 'plus' (needs 2 items)
```

Each operation has specific requirements:

| Operation | Minimum Stack Size |
|-----------|-------------------|
| `dup` | 1 |
| `drop` | 1 |
| `swap` | 2 |
| `over` | 2 |
| `rot` | 3 |
| `plus`, `minus`, etc. | 2 |

### Preventing Underflow

1. **Check depth before operating:**
   ```bash
   > depth 2 ge? [swap] [] if
   ```

2. **Use .s to inspect:**
   ```bash
   > .s
   Stack: []  # Empty! Don't call swap
   ```

3. **Use try for error handling:**
   ```bash
   > [swap] try error? ["Not enough items"] [] if
   ```

### Common Underflow Scenarios

```bash
# Forgetting the stack is empty after clear
> .clear
> dup
Error: Stack underflow in 'dup'

# Off-by-one in loops
> 1 2 3 [drop] 4 times
Error: Stack underflow in 'drop'  # Only 3 items, tried 4 drops

# Operations in definitions consuming more than expected
> [swap drop] :nip
> 1 nip  # Only 1 item, nip needs 2
Error: Stack underflow in 'swap'
```

---

## Keyboard Shortcuts for Stack Manipulation

The REPL provides shortcuts for moving data between the stack and input line:

| Shortcut | Action |
|----------|--------|
| **Alt+Up** | Pop from stack, insert into input |
| **Alt+Down** | Push first word from input to stack |
| **Alt+A** | Push ALL words from input to stack |
| **Alt+a** | Pop ALL from stack to input |
| **Alt+k** | Clear (kill) the stack |
| **Alt+c** | Copy top of stack to clipboard |
| **Alt+x** | Cut top of stack to clipboard |

Example workflow:

```bash
> file1.txt file2.txt file3.txt    # Push three files
> Alt+Up                            # Pop file3.txt to input
file3.txt                           # Now in input line
> cat                               # Complete the command
```

---

## Summary: Quick Reference

```
dup    ( a -- a a )       Duplicate top
drop   ( a -- )           Discard top
swap   ( a b -- b a )     Swap top two
over   ( a b -- a b a )   Copy second to top
rot    ( a b c -- b c a ) Rotate top three
depth  ( -- n )           Push stack size

# From stdlib:
nip    ( a b -- b )       Drop second
tuck   ( a b -- b a b )   Copy top below second
-rot   ( a b c -- c a b ) Reverse rotate
2drop  ( a b -- )         Drop top two
2dup   ( a -- a a a )     [Note: stdlib version triplicates]

# Inspection:
.s     Show stack contents
.stack Show stack contents (alias)
```

---

## See Also

- [README.md](../README.md) - Overview and quick start
- [extending-stdlib.md](extending-stdlib.md) - Adding custom definitions
