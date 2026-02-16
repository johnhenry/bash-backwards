# hsab Language Reference

hsab (Hash Backwards) is a stack-based postfix shell. Values push to the stack, commands pop arguments and push results.

**See also:**
- [Getting Started](getting-started.md) - Installation and first commands
- [Interactive REPL](repl.md) - Keyboard shortcuts, visual hints, exploration workflow
- [Shell Guide](shell.md) - Running commands, pipelines, variables
- [Configuration](config.md) - Environment variables and settings

## Table of Contents

1. [Syntax](#syntax)
2. [Operators](#operators)
3. [Value Types](#value-types)
4. [Stack Operations](#stack-operations)
5. [Arithmetic](#arithmetic)
6. [Predicates](#predicates)
7. [String Operations](#string-operations)
8. [Path Operations](#path-operations)
9. [List Operations](#list-operations)
10. [Control Flow](#control-flow)
11. [Structured Data](#structured-data)
12. [Serialization](#serialization)
13. [Aggregations](#aggregations)
14. [Vector Operations](#vector-operations)
15. [Combinators](#combinators)
16. [Async and Concurrency](#async-and-concurrency)
17. [HTTP Client](#http-client)
18. [Shell Builtins](#shell-builtins)
19. [File Operations](#file-operations)
20. [Encoding and Hashing](#encoding-and-hashing)
21. [BigInt Operations](#bigint-operations)
22. [Module System](#module-system)
23. [Plugin System](#plugin-system)
24. [Meta Commands](#meta-commands)
25. [REPL Commands](#repl-commands)

---

## Syntax

### Literals

Bare words push directly to the stack:

```hsab
hello               # Pushes "hello"
/path/to/file       # Pushes "/path/to/file"
-la                 # Pushes "-la" (flags are literals)
```

### Strings

#### Double-Quoted Strings

Support escape sequences and variable interpolation:

```hsab
"hello world"           # String with spaces
"line1\nline2"          # Escape sequences: \n \t \r \\ \"
"home is $HOME"         # Variable interpolation
```

Escape sequences:
- `\n` - newline
- `\t` - tab
- `\r` - carriage return
- `\\` - backslash
- `\"` - double quote
- `\$` - literal dollar sign
- `\e` - escape character
- `\x##` - hex byte
- `\0##` - octal byte

#### Single-Quoted Strings

Literal strings, no interpolation:

```hsab
'hello $HOME'           # Literal: hello $HOME (no expansion)
```

#### Triple-Quoted Strings

Multiline strings:

```hsab
"""
This is a
multiline string
"""

'''
Single-quoted
multiline literal
'''
```

### Numbers

```hsab
42                  # Integer
3.14                # Float
-17                 # Negative integer
1e10                # Scientific notation
```

### Blocks

Deferred execution units enclosed in square brackets:

```hsab
[echo hello]            # A block (not executed yet)
[1 2 plus]              # Block with arithmetic
[dup mul]               # Block referencing stack
```

Blocks can be:
- Applied with `@`
- Passed to control structures
- Stored as definitions

### Vectors (Lists)

```hsab
[1, 2, 3]               # List with commas
[1 2 3]                 # List with spaces (same result)
["a", "b", "c"]         # List of strings
```

### Records (Maps)

```hsab
{name: "Alice", age: 30}
{key: value, nested: {inner: data}}
```

### Comments

```hsab
# This is a comment
echo hello  # Inline comment
```

### Variables

```hsab
$HOME                   # Environment variable
${HOME}                 # Braced form
$?                      # Last exit code
```

### Definitions

Store a block with a name:

```hsab
[dup mul] :square       # Define 'square' as dup then mul
5 square                # Use it: pushes 25
```

### Scoped Assignments

```hsab
VAR=value; command      # VAR set for command duration only
A=1 B=2; [echo $A $B]   # Multiple assignments
```

---

## Operators

### Pipe (`|`)

Pipe output from producer to consumer:

```hsab
ls [grep txt] |         # ls | grep txt
```

### Redirects

#### Standard Output

```hsab
[echo hello] [file.txt] >       # Overwrite file
[echo hello] [file.txt] >>      # Append to file
```

#### Standard Input

```hsab
[sort] [data.txt] <             # Read from file
```

#### Standard Error

```hsab
[cmd] [errors.log] 2>           # Redirect stderr
[cmd] [errors.log] 2>>          # Append stderr
```

#### Both Streams

```hsab
[cmd] [output.log] &>           # Redirect stdout and stderr
[cmd] 2>&1                      # Merge stderr into stdout
```

### Background (`&`)

```hsab
[long-running-task] &           # Run in background
```

### Apply (`@`)

Execute a block:

```hsab
[echo hello] @                  # Execute the block
```

### Logic Operators

```hsab
[cmd1] [cmd2] &&                # Run cmd2 only if cmd1 succeeds
[cmd1] [cmd2] ||                # Run cmd2 only if cmd1 fails
```

---

## Value Types

hsab has the following value types (from `Value` enum):

| Type | Description | Example |
|------|-------------|---------|
| `Literal` | A string value | `"hello"`, `file.txt` |
| `Output` | Command output | Result of `ls` |
| `Number` | Floating-point number | `42`, `3.14` |
| `Bool` | Boolean | `true`, `false` |
| `Nil` | Empty/null value | Empty output |
| `List` | Ordered collection | `[1, 2, 3]` |
| `Map` | Key-value pairs | `{name: "Alice"}` |
| `Table` | Columnar data | CSV/TSV data |
| `Block` | Deferred expressions | `[echo hello]` |
| `Marker` | Stack boundary | Used by `spread`/`collect` |
| `Error` | Structured error | `{kind, message, code}` |
| `Bytes` | Raw binary data | Hash output |
| `BigInt` | Arbitrary precision integer | Cryptographic values |
| `Media` | Image/binary content | PNG, JPEG data |
| `Link` | Hyperlink (OSC 8) | Clickable terminal links |
| `Future` | Async computation handle | Background task |

### Type Introspection

```hsab
42 typeof               # "Number"
"hello" typeof          # "Literal"
[1, 2, 3] typeof        # "List"
```

---

## Stack Operations

| Operation | Stack Effect | Description |
|-----------|--------------|-------------|
| `dup` | `a -- a a` | Duplicate top |
| `swap` | `a b -- b a` | Swap top two |
| `drop` | `a b -- a` | Remove top |
| `over` | `a b -- a b a` | Copy second to top |
| `rot` | `a b c -- b c a` | Rotate top three |
| `depth` | `-- n` | Push stack size |
| `dig` | `... n -- ...` | Pull Nth item to top |
| `bury` | `a ... n -- ...` | Push top down to Nth position |

`dig` and `bury` provide deep stack access. `pick` is an alias for `dig`, `roll` is an alias for `bury`.

```hsab
1 2 3 4 5  3 dig           # Pulls 3rd from top -> 1 2 4 5 3
1 2 3 4 5  3 bury          # Buries top down 3 -> 1 2 5 3 4
```

### Stack Snapshots

```hsab
"name" snapshot             # Save stack state with name
snapshot                    # Auto-name, returns name
"name" snapshot-restore     # Restore saved state
snapshot-list               # List all snapshots
"name" snapshot-delete      # Delete snapshot
snapshot-clear              # Clear all snapshots
```

---

## Arithmetic

### Basic Operations

| Operation | Stack Effect | Description |
|-----------|--------------|-------------|
| `plus` | `a b -- (a+b)` | Addition |
| `minus` | `a b -- (a-b)` | Subtraction |
| `mul` | `a b -- (a*b)` | Multiplication |
| `div` | `a b -- (a/b)` | Division (float) |
| `mod` | `a b -- (a%b)` | Modulo |
| `idiv` | `a b -- (a/b)` | Integer division |
| `pow` | `a b -- (a^b)` | Exponentiation |

### Math Functions

| Operation | Stack Effect | Description |
|-----------|--------------|-------------|
| `sqrt` | `n -- sqrt(n)` | Square root |
| `floor` | `n -- floor(n)` | Round down |
| `ceil` | `n -- ceil(n)` | Round up |
| `round` | `n -- round(n)` | Round to nearest |
| `abs` | `n -- \|n\|` | Absolute value |
| `negate` | `n -- -n` | Negate |
| `log-base` | `val base -- log` | Logarithm with arbitrary base |
| `max-of` | `a b -- max` | Maximum of two numbers |
| `min-of` | `a b -- min` | Minimum of two numbers |

### Dynamic Operator Patterns

Tokens like `3+` or `10log` are expanded at parse time into a number and an operation. This provides a concise shorthand for common arithmetic:

| Pattern | Expands to | Example |
|---------|-----------|---------|
| `<n>+` | `n plus` | `3+` = push 3, then add |
| `<n>-` | `n minus` | `5-` = push 5, then subtract |
| `<n>*` | `n mul` | `2*` = push 2, then multiply |
| `<n>/` | `n div` | `4/` = push 4, then divide |
| `<n>%` | `n mod` | `3%` = push 3, then modulo |
| `<n>log` | `n log-base` | `10log` = log base 10 |
| `<n>pow` | `n pow` | `2pow` = raise to power of 2 |

```hsab
5 3+                    # 8 (same as 5 3 plus)
10 2/                   # 5 (same as 10 2 div)
100 10log               # 2 (log base 10 of 100)
3 2pow                  # 9 (3 raised to power 2)
7 2*                    # 14
```

### Unicode Operator Aliases

| Symbol | Alias for | Description |
|--------|-----------|-------------|
| `Σ` | `sum` | Sum a list |
| `Π` | `product` | Product of a list |
| `÷` | `div` | Division |
| `⋅` | `mul` | Multiplication |
| `√` | `sqrt` | Square root |
| `∅` | push `nil` | Empty/null value |
| `≠` | `ne?` | String not equal |
| `≤` | `le?` | Less than or equal |
| `≥` | `ge?` | Greater than or equal |
| `μ` | `avg` | Mean (average) |

```hsab
10 3 ÷                  # 3.333...
4 5 ⋅                   # 20
16 √                    # 4
[1, 2, 3, 4, 5] Σ      # 15
[1, 2, 3, 4, 5] Π      # 120
5 10 ≤                  # Exit 0 (true)
```

### Examples

```hsab
5 3 plus                # 8
10 3 minus              # 7
4 5 mul                 # 20
10 3 div                # 3.333...
10 3 mod                # 1
2 10 pow                # 1024
16 sqrt                 # 4
3.7 floor               # 3
3.2 ceil                # 4
100 10 log-base         # 2
-5 abs                  # 5
5 negate                # -5
3 7 max-of              # 7
3 7 min-of              # 3
```

---

## Predicates

Predicates set the exit code: 0 for true, 1 for false.

### Numeric Comparisons

| Predicate | Description |
|-----------|-------------|
| `=?` | Equal |
| `!=?` | Not equal |
| `lt?` | Less than |
| `gt?` | Greater than |
| `le?` | Less than or equal |
| `ge?` | Greater than or equal |

```hsab
5 5 =?                  # Exit 0 (equal)
5 10 lt?                # Exit 0 (5 < 10)
10 5 gt?                # Exit 0 (10 > 5)
```

### String Comparisons

| Predicate | Description |
|-----------|-------------|
| `eq?` | Strings equal |
| `ne?` | Strings not equal |

```hsab
"hello" "hello" eq?     # Exit 0
"hello" "world" ne?     # Exit 0
```

### File Predicates

| Predicate | Description |
|-----------|-------------|
| `file?` | Is regular file |
| `dir?` | Is directory |
| `exists?` | Path exists |

```hsab
"/etc/passwd" file?     # Exit 0 if file exists
"/tmp" dir?             # Exit 0 if directory
"./missing" exists?     # Exit 1 if not found
```

### String Predicates

| Predicate | Description |
|-----------|-------------|
| `empty?` | String is empty |
| `contains?` | String contains substring |
| `starts?` | String starts with prefix |
| `ends?` | String ends with suffix |

```hsab
"hello world" "wor" contains?   # Exit 0 (contains "wor")
"hello" "he" starts?            # Exit 0 (starts with "he")
"file.txt" ".txt" ends?         # Exit 0 (ends with ".txt")
"hello" "xyz" contains?         # Exit 1 (no match)
```

### Type Predicates

| Predicate | Description |
|-----------|-------------|
| `nil?` | Value is nil (non-destructive) |
| `error?` | Value is error (non-destructive) |
| `has?` | Record has key |

```hsab
"/nonexistent" cd nil?          # Exit 0 (cd failed, pushed nil)
42 nil?                         # Exit 1 (not nil)
[throw "oops"] try error?       # Exit 0 (caught error)
```

---

## String Operations

| Operation | Description | Example |
|-----------|-------------|---------|
| `len` | String length | `"hello" len` -> `5` |
| `slice` | Substring | `"hello" 1 3 slice` -> `"ell"` |
| `indexof` | Find position | `"hello" "l" indexof` -> `2` |
| `str-replace` | Replace all | `"hello" "l" "L" str-replace` -> `"heLLo"` |
| `split1` | Split at first | `"a.b.c" "." split1` -> `"a"` `"b.c"` |
| `rsplit1` | Split at last | `"a.b.c" "." rsplit1` -> `"a.b"` `"c"` |
| `format` | Interpolate | `"Alice" "Hello, {{}}!" format` -> `"Hello, Alice!"` |

### Format Placeholders

```hsab
"Alice" "Hello, {{}}!" format           # "Hello, Alice!"
"Bob" "Alice" "{{1}} meets {{0}}" format  # "Alice meets Bob"
```

---

## Path Operations

| Operation | Description | Example |
|-----------|-------------|---------|
| `path-join` | Join paths | `"/dir" "file.txt" path-join` -> `"/dir/file.txt"` |
| `dirname` | Get directory | `"/path/to/file.txt" dirname` -> `"/path/to"` |
| `basename` | Get filename | `"/path/to/file.txt" basename` -> `"file.txt"` |
| `extname` | Get extension | `"/path/file.txt" extname` -> `".txt"` |
| `realpath` | Canonical path | `"../file" realpath` -> `"/absolute/path/file"` |
| `suffix` | Add suffix | `"file" "_bak" suffix` -> `"file_bak"` |
| `reext` | Replace extension | `"file.txt" ".md" reext` -> `"file.md"` |

---

## List Operations

### Spreading and Collecting

```hsab
"a\nb\nc" spread        # Push marker, then "a", "b", "c"
marker                  # Push explicit marker
collect                 # Gather items to marker into list
```

### Iteration

```hsab
spread [echo] each              # Apply block to each item
spread [2 mul] map              # Transform each item
spread [10 lt?] filter          # Keep items < 10
spread [10 lt?] keep            # Same as filter
spread [10 lt?] reject          # Remove items < 10
```

### Extended Spread Operations

| Operation | Description |
|-----------|-------------|
| `fields` | Spread record values |
| `fields-keys` | Spread record keys |
| `spread-head` | Spread first N items |
| `spread-tail` | Spread last N items |
| `spread-n` | Spread exactly N items |
| `spread-to` | Spread until delimiter |

---

## Control Flow

### Conditional

```hsab
[condition] [then-block] [else-block] if
```

Example:
```hsab
[5 10 lt?] [echo "less"] [echo "greater or equal"] if
```

### Loops

#### Times Loop
```hsab
5 [echo "hello"] times          # Repeat 5 times
```

#### While Loop
```hsab
[condition] [body] while        # While condition succeeds
```

#### Until Loop
```hsab
[condition] [body] until        # Until condition succeeds
```

#### Break
```hsab
break                           # Exit current loop early
```

---

## Structured Data

### Records

```hsab
"name" "Alice" "age" 30 record  # Create record
record "name" get               # Get field: "Alice"
record "address.city" get       # Deep get with dot notation
record "name" "Bob" set         # Set field
record "name" del               # Delete field
record "name" has?              # Check field exists
record keys                     # Get all keys
record values                   # Get all values
rec1 rec2 merge                 # Merge records
```

### Tables

```hsab
marker rec1 rec2 rec3 table     # Create table from records
table [predicate] where         # Filter rows
table [predicate] reject-where  # Keep rows that DON'T match
table "column" sort-by          # Sort by column
table "col1" "col2" select      # Select columns
table first                     # First row
table last                      # Last row
table 5 nth                     # Nth row
table "column" group-by         # Group by column
```

### List Transforms

```hsab
list unique                     # Remove duplicates
list reverse                    # Reverse order
list flatten                    # Flatten nested lists
list duplicates                 # Items appearing more than once
```

### Error Handling

```hsab
[risky-operation] try           # Catch errors
value error?                    # Check if error (exit 0/1)
"message" throw                 # Raise error
```

---

## Serialization

### Text to Structured

| Operation | Description |
|-----------|-------------|
| `into-json` | Parse JSON string |
| `into-csv` | Parse CSV text |
| `into-tsv` | Parse TSV text |
| `into-delimited` | Parse with custom delimiter |
| `into-lines` | Split into list of lines |
| `into-kv` | Parse key=value pairs |
| `json` | Alias for `into-json` |

### Structured to Text

| Operation | Description |
|-----------|-------------|
| `to-json` / `unjson` | Convert to JSON |
| `to-csv` | Convert to CSV |
| `to-tsv` | Convert to TSV |
| `to-delimited` | Convert with custom delimiter |
| `to-lines` | Join list with newlines |
| `to-kv` | Convert to key=value format |

### File I/O

```hsab
"data.json" open                # Auto-parse by extension
data "output.csv" save          # Auto-format by extension
```

Supported extensions: `.json`, `.csv`, `.tsv`, `.toml`, `.yaml`

---

## Aggregations

| Operation | Description |
|-----------|-------------|
| `sum` | Sum of numbers |
| `avg` | Average of numbers |
| `min` | Minimum value |
| `max` | Maximum value |
| `count` | Count items |
| `reduce` | Fold with initial value and block |

```hsab
[1, 2, 3, 4, 5] sum             # 15
[1, 2, 3, 4, 5] avg             # 3
[1, 2, 3, 4, 5] min             # 1
[1, 2, 3, 4, 5] max             # 5
[1, 2, 3, 4, 5] count           # 5

# Reduce: list init [block] reduce
[1, 2, 3] 0 [plus] reduce       # 6 (sum via reduce)
```

### Statistical Functions

| Operation | Stack Effect | Description |
|-----------|--------------|-------------|
| `product` | `[nums] -- n` | Multiply all elements |
| `median` | `[nums] -- n` | Middle value (sorted) |
| `mode` | `[nums] -- n` | Most frequent value |
| `modes` | `[nums] -- [nums]` | All values with highest frequency |
| `variance` | `[nums] -- n` | Population variance |
| `sample-variance` | `[nums] -- n` | Sample variance (N-1) |
| `stdev` | `[nums] -- n` | Population standard deviation |
| `sample-stdev` | `[nums] -- n` | Sample standard deviation (N-1) |
| `percentile` | `[nums] p -- n` | Value at percentile p (0.0-1.0) |
| `five-num` | `[nums] -- [5 nums]` | Five-number summary [min, Q1, median, Q3, max] |
| `sort-nums` | `[nums] -- [nums]` | Sort numerically |

```hsab
[1, 2, 3, 4, 5] product        # 120
[1, 2, 3, 4, 5] median         # 3
[1, 2, 2, 3, 3, 3] mode        # 3
[1, 2, 2, 3, 3] modes          # [2, 3] (both appear twice)
[2, 4, 4, 4, 5, 5, 7, 9] variance      # 4.25
[2, 4, 4, 4, 5, 5, 7, 9] sample-variance  # 4.857...
[2, 4, 4, 4, 5, 5, 7, 9] stdev         # 2.0615...
[1, 2, 3, 4, 5] 0.5 percentile         # 3 (50th percentile = median)
[1, 2, 3, 4, 5] five-num               # [1, 2, 3, 4, 5]
[3, 1, 2] sort-nums                    # [1, 2, 3]
```

---

## Vector Operations

For working with embeddings and numerical vectors:

| Operation | Description |
|-----------|-------------|
| `dot-product` | Dot product of two vectors |
| `magnitude` | L2 norm |
| `normalize` | Convert to unit vector |
| `cosine-similarity` | Similarity measure (-1 to 1) |
| `euclidean-distance` | Distance between vectors |

```hsab
[1, 0, 0] [0, 1, 0] dot-product         # 0
[3, 4] magnitude                         # 5
[3, 4] normalize                         # [0.6, 0.8]
[1, 0] [0, 1] cosine-similarity         # 0
[0, 0] [3, 4] euclidean-distance        # 5
```

---

## Combinators

### Fanout

Run value through multiple blocks:

```hsab
10 [2 mul] [3 plus] fanout      # 20, 13
```

### Zip

Pair elements from two lists:

```hsab
[1, 2, 3] ["a", "b", "c"] zip   # [[1,"a"], [2,"b"], [3,"c"]]
```

### Cross

Cartesian product:

```hsab
[1, 2] ["a", "b"] cross         # [[1,"a"], [1,"b"], [2,"a"], [2,"b"]]
```

### Retry

Retry until success:

```hsab
3 [unreliable-operation] retry  # Try up to 3 times
```

### Compose

Combine blocks into pipeline:

```hsab
[op1] [op2] [op3] compose       # [op1 op2 op3]
```

### Utility Combinators

```hsab
value [block] tap               # Apply block, keep original value
a b [block] dip                 # Apply block to second item
```

---

## Async and Concurrency

### Creating Futures

```hsab
[long-running-task] async       # Returns Future
100 delay                       # Sleep 100ms (blocking)
100 delay-async                 # Sleep 100ms (non-blocking Future)
```

### Awaiting Futures

```hsab
future await                    # Block until complete, get result
[futures] await-all             # Await list of futures
future1 future2 2 future-await-n # Await N futures from stack
```

### Future Management

```hsab
future future-status            # "pending", "completed", "failed", "cancelled"
future future-result            # {ok: value} or {err: message}
future future-cancel            # Cancel a running future
future [transform] future-map   # Transform result without awaiting
```

### Parallel Execution

```hsab
[[cmd1] [cmd2]] parallel        # Run in parallel, wait for all
[[cmd1] [cmd2]] 2 parallel-n    # Limit concurrency to 2
[[cmd1] [cmd2]] race            # Return first to complete
[futures] future-race           # Race existing futures
```

### Parallel Map

Apply a block to each item in a list with bounded concurrency. Each worker thread receives one item on its stack, runs the block, and returns the top-of-stack result. Results are collected in the original order.

```hsab
# Signature: list [block] N parallel-map -> [results]

# Double each number using 4 threads
[1 2 3 4 5 6 7 8] [2 mul] 4 parallel-map   # [2, 4, 6, 8, 10, 12, 14, 16]

# Fetch multiple URLs concurrently (2 at a time)
["https://a.com" "https://b.com" "https://c.com"] [fetch] 2 parallel-map

# Process files in parallel (up to 8 threads)
["a.txt" "b.txt" "c.txt"] [open] 8 parallel-map
```

| Param | Type | Description |
|-------|------|-------------|
| list | List or Block | Items to process (block is evaluated first) |
| block | Block | Applied to each item |
| N | Number | Max concurrent threads |

Errors inside a worker thread are captured as `Value::Error` in the result list rather than aborting the whole operation.

### Background Jobs

```hsab
[cmd1] [cmd2] 2 fork            # Background N blocks
```

### Process Substitution

```hsab
[cmd] subst                     # Create temp file with output
[cmd] fifo                      # Create named pipe with output
```

### Resource Limits

```hsab
5 [long-task] timeout           # Kill after 5 seconds
```

---

## HTTP Client

### Basic Requests

```hsab
"https://api.example.com" fetch             # GET request
"https://api.example.com" "POST" fetch      # POST request
body "https://api.example.com" "POST" fetch # POST with body
```

### Response Types

```hsab
"https://api.example.com" fetch             # Returns body (auto-parses JSON)
"https://api.example.com" fetch-status      # Returns status code
"https://api.example.com" fetch-headers     # Returns headers as Map
```

### With Headers

```hsab
{Authorization: "Bearer token"} body "https://api.example.com" "POST" fetch
```

### Practical Examples

**Fetching JSON API data:**

```hsab
# Get user data from a REST API
"https://jsonplaceholder.typicode.com/users/1" fetch
# Returns: {id: 1, name: "Leanne Graham", email: "..."}

# Extract specific field
"https://jsonplaceholder.typicode.com/users/1" fetch "name" get
# Returns: "Leanne Graham"
```

**POST with JSON body:**

```hsab
# Create a new resource
'{"title": "foo", "body": "bar", "userId": 1}' json
"https://jsonplaceholder.typicode.com/posts" "POST" fetch
# Returns: {id: 101, title: "foo", body: "bar", userId: 1}
```

**Checking response status:**

```hsab
# Verify a resource exists
"https://api.example.com/resource" fetch-status
200 =? ["Resource found" echo] ["Not found" echo] if
```

**Parallel API calls:**

```hsab
# Fetch multiple resources concurrently
[
  ["https://api.example.com/users" fetch]
  ["https://api.example.com/posts" fetch]
  ["https://api.example.com/comments" fetch]
] parallel
# Stack now has all three responses
```

**Error handling:**

```hsab
# Handle network errors gracefully
["https://api.example.com/data" fetch] try
error? [
  "API request failed" echo
  drop  # Remove error from stack
] [
  "data" get  # Process successful response
] if
```

---

## Shell Builtins

### Navigation

```hsab
/tmp cd                 # Change directory (or .cd)
pwd                     # Print working directory
/tmp pushd              # Push directory
popd                    # Pop directory
dirs                    # Show directory stack
```

### Environment

```hsab
VAR=value .export       # Set environment variable
VAR .unset              # Remove variable
.env                    # List all variables
```

### I/O

```hsab
hello echo              # Print "hello" (or .echo)
"Hello %s" name printf  # Formatted print (or .printf)
varname read            # Read line into variable (or .read)
```

### Job Control

```hsab
.jobs                   # List background jobs
%1 .fg                  # Bring job to foreground
%1 .bg                  # Resume in background
.wait                   # Wait for all jobs
%1 .wait                # Wait for specific job
%1 .kill                # Kill job
%1 -9 .kill             # Kill with signal
```

### Tests

```hsab
file.txt -f test        # Test if file
/tmp -d test            # Test if directory
5 10 -lt test           # Numeric comparison
```

### Other Builtins

```hsab
true                    # Exit 0
false                   # Exit 1
.exit                   # Exit shell
0 .exit                 # Exit with code
file.txt vim .tty       # Run interactive command
file.hsab .source       # Execute file in current context
ls .which               # Find executable path
ls .type                # Show how word resolves
.hash                   # Show/manage command cache
```

---

## File Operations

hsab provides **stack-native** file operations that return useful values instead of being side-effect only. All operations return `nil` on error, enabling graceful error handling in pipelines.

See also: [Shell Guide: Stack-Native Operations](shell.md#stack-native-shell-operations)

### Quick Reference

| Operation | Stack Effect | Description |
|-----------|--------------|-------------|
| `touch` | `path -- path\|nil` | Create file, return canonical path |
| `mkdir` | `path -- path\|nil` | Create directory |
| `mkdir-p` | `path -- path\|nil` | Create directory tree (parents) |
| `mktemp` | `-- path` | Create temp file, return path |
| `mktemp-d` | `-- path` | Create temp directory, return path |
| `cp` | `src dst -- dst\|nil` | Copy file, return destination |
| `mv` | `src dst -- dst\|nil` | Move/rename, return destination |
| `rm` | `path -- count\|nil` | Remove file(s), return count deleted |
| `rm-r` | `path -- count\|nil` | Remove recursively, return count |
| `ln` | `target link -- link\|nil` | Create symlink, return link path |
| `realpath` | `path -- path\|nil` | Resolve to canonical absolute path |
| `ls` | `[pattern] -- [files]` | List directory as vector |
| `glob` | `pattern -- [paths]` | Glob match, return vector |
| `which` | `cmd -- path\|nil` | Find executable path |
| `cd` | `[path] -- path\|nil` | Change directory, return new path |
| `extname` | `path -- ext` | Extract file extension |

### File Creation

#### touch

Create a file and return its canonical path:

```hsab
# Create a file
"newfile.txt" touch             # "/abs/path/to/newfile.txt"

# Chain operations
"data.txt" touch dup            # Create and keep path
"Hello" swap write              # Write content to it

# Error returns nil
"/nonexistent/dir/file.txt" touch  # nil (parent doesn't exist)
nil? ["Failed to create file" echo] [] if
```

#### mkdir / mkdir-p

Create directories:

```hsab
# Single directory
"mydir" mkdir                   # "/abs/path/to/mydir"

# Create parent directories
"a/b/c/d" mkdir-p               # "/abs/path/to/a/b/c/d"

# Use returned path immediately
"project" mkdir                 # Returns path
"src" path-join mkdir           # Create project/src
```

#### mktemp / mktemp-d

Create temporary files and directories:

```hsab
# Temp file (auto-generated unique name)
mktemp                          # "/tmp/hsab-abc123"
"temporary data" swap write

# Temp directory
mktemp-d                        # "/tmp/hsab-dir-xyz789"
"/file.txt" path-join touch     # Create file inside
```

### File Operations

#### cp (Copy)

Copy files and return the destination path:

```hsab
# Copy file
"src.txt" "dst.txt" cp          # "dst.txt"

# Copy to directory
"file.txt" "backup/" cp         # "backup/file.txt"

# Chain: copy and read the copy
"original.txt" "copy.txt" cp cat

# Error handling
"missing.txt" "dst.txt" cp      # nil
nil? ["Copy failed" echo] [] if
```

#### mv (Move/Rename)

Move or rename files:

```hsab
# Rename file
"old.txt" "new.txt" mv          # "new.txt"

# Move to directory
"file.txt" "/tmp/" mv           # "/tmp/file.txt"

# Rename with processing
"data.txt" dup ".bak" suffix mv # "data.txt.bak"
```

#### rm / rm-r (Remove)

Remove files and return the count deleted:

```hsab
# Remove single file
"temp.txt" rm                   # 1 (one file deleted)

# Remove with glob pattern
"*.tmp" rm                      # 5 (five files deleted)

# Show what was deleted
*.log rm "Deleted" swap suffix echo  # "Deleted 3"

# Recursive removal
"old-project/" rm-r             # 42 (total items deleted)

# Non-existent file returns nil
"missing.txt" rm                # nil
```

#### ln (Symlink)

Create symbolic links:

```hsab
# Create symlink (target first, then link name)
"/usr/local/bin/python3" "python" ln  # "python"

# Relative symlink
"../shared/config" ".config" ln
```

### Path Operations

#### realpath

Resolve to canonical absolute path:

```hsab
"../file.txt" realpath          # "/home/user/parent/file.txt"
"~/Documents" realpath          # "/home/user/Documents"
"./script.sh" realpath          # "/current/dir/script.sh"

# Non-existent path returns nil
"/no/such/path" realpath        # nil
```

#### cd

Change directory and return the new path:

```hsab
# Change to directory
"/tmp" cd                       # "/tmp"

# No argument goes to home
cd                              # "/home/user"

# Tilde expansion
"~/Documents" cd                # "/home/user/Documents"

# Invalid directory returns nil
"/nonexistent" cd               # nil
"Cargo.toml" cd                 # nil (not a directory)
```

#### extname

Extract file extension:

```hsab
"/path/to/file.txt" extname     # ".txt"
"archive.tar.gz" extname        # ".gz"
"Makefile" extname              # "" (no extension)
```

### Directory Listing

#### ls

List directory contents as a vector:

```hsab
# List current directory
ls                              # ["file1.txt", "file2.rs", "dir/"]

# List specific directory
"/tmp" ls                       # ["temp1", "temp2", ...]

# With glob pattern
"*.rs" ls                       # ["main.rs", "lib.rs"]

# Process listing
"src/" ls spread                # Spread onto stack
[-f test] keep                  # Filter to files only
[wc -l] each                    # Count lines in each
```

#### glob

Match glob patterns:

```hsab
# Simple glob
"*.txt" glob                    # ["a.txt", "b.txt", ...]

# Recursive glob
"**/*.rs" glob                  # All .rs files recursively

# Multiple patterns
"{*.rs,*.toml}" glob            # .rs and .toml files
```

#### which

Find executable path:

```hsab
"python3" which                 # "/usr/bin/python3"
"cargo" which                   # "/home/user/.cargo/bin/cargo"

# Not found returns nil
"nonexistent-cmd" which         # nil
```

### Error Handling

All stack-native operations return `nil` on error:

```hsab
# Check for errors with nil?
"file.txt" touch
nil? [
    "Failed to create file" echo
] [
    "Created:" swap suffix echo
] if

# Use in pipelines (nil propagates)
"src.txt" "dst.txt" cp          # nil if failed
nil? [] [cat] if                # Only cat if copy succeeded

# Try/catch for more control
["missing.txt" cat] try
error? ["File not found" echo] [] if
```

### Compositional Pipelines

Stack-native operations enable functional pipelines:

```hsab
# Create, write, and read back
"output.txt" touch              # Returns path
dup "Hello, World!" swap write  # Write to it
cat                             # Read it back

# Batch file processing
"*.txt" glob spread             # All .txt files on stack
[dup ".bak" suffix cp] each     # Copy each to .bak

# Conditional operations
"config.json" dup -f test       # Check if exists
[[cat json] @] [["{}" json] @] if  # Parse or use default
```

See [Shell Guide](shell.md) for more examples and patterns.

---

## Encoding and Hashing

### Base64

```hsab
value to-base64                 # Encode to base64
encoded from-base64             # Decode from base64
```

**Examples:**

```hsab
# Encode a string
"Hello, World!" to-base64
# Returns: "SGVsbG8sIFdvcmxkIQ=="

# Decode back
"SGVsbG8sIFdvcmxkIQ==" from-base64
# Returns: "Hello, World!"

# Encode JSON for URL-safe transmission
'{"user": "alice", "token": "secret"}' to-base64
```

### Hex

```hsab
bytes to-hex                    # Bytes to hex string
"deadbeef" from-hex             # Hex string to bytes
```

**Examples:**

```hsab
# Convert bytes to hex
"hello" as-bytes to-hex
# Returns: "68656c6c6f"

# Convert hex back to bytes and string
"68656c6c6f" from-hex to-string
# Returns: "hello"

# Useful for displaying binary data
"file.bin" cat as-bytes to-hex
```

### Bytes

```hsab
"hello" as-bytes                # String to bytes (UTF-8)
"hello" to-bytes                # Alias for as-bytes
bytes to-string                 # Bytes to string
```

**Examples:**

```hsab
# Get byte representation
"Hello" as-bytes
# Returns: Bytes([72, 101, 108, 108, 111])

# Chain with hex encoding
"Secret" as-bytes to-hex
# Returns: "536563726574"
```

### Hash Functions (SHA-2)

```hsab
"data" sha256                   # SHA-256 hash
"data" sha384                   # SHA-384 hash
"data" sha512                   # SHA-512 hash
"file.txt" sha256-file          # Hash file contents
```

**Examples:**

```hsab
# Hash a password (don't actually do this for real passwords!)
"password123" sha256 to-hex
# Returns: "ef92b778bafe771e89245b89ecbc08a44a4e166c06659911881f383d4473e94f"

# Verify file integrity
"download.zip" sha256-file to-hex
"expected_hash" eq? ["File OK" echo] ["Corrupted!" echo] if

# Hash multiple values
"salt" "password" + sha256 to-hex
```

### Hash Functions (SHA-3)

```hsab
"data" sha3-256                 # SHA3-256 hash
"data" sha3-384                 # SHA3-384 hash
"data" sha3-512                 # SHA3-512 hash
"file.txt" sha3-256-file        # Hash file contents
```

**Examples:**

```hsab
# SHA-3 for newer applications
"important data" sha3-256 to-hex

# Compare SHA-2 vs SHA-3
"test" sha256 to-hex      # SHA-2
"test" sha3-256 to-hex    # SHA-3 (different algorithm, same output length)

# Hash a config file
"config.yaml" sha3-256-file to-hex _hash local
"Config hash: $_hash" echo
```

---

## BigInt Operations

Arbitrary precision unsigned integers for cryptographic operations. Use when you need numbers larger than 64-bit integers can represent.

### Conversion

```hsab
"12345678901234567890" to-bigint    # Create BigInt
```

**Examples:**

```hsab
# Create BigInts from strings
"999999999999999999999999999" to-bigint
# Returns: BigInt(999999999999999999999999999)

# Convert from hex
"ffffffffffffffff" from-hex to-bigint
```

### Arithmetic

| Operation | Description |
|-----------|-------------|
| `big-add` | Addition |
| `big-sub` | Subtraction |
| `big-mul` | Multiplication |
| `big-div` | Division |
| `big-mod` | Modulo |
| `big-pow` | Exponentiation |

**Examples:**

```hsab
# Add two large numbers
"99999999999999999999" to-bigint
"11111111111111111111" to-bigint
big-add
# Returns: BigInt(111111111111111111110)

# Modular exponentiation (useful for crypto)
"2" to-bigint           # base
"256" to-bigint         # exponent
big-pow
# Returns: BigInt(very large number)

# Calculate factorial of 100
"1" to-bigint _acc local
100 [
  dup to-bigint $_acc big-mul _acc local
  1 minus
  dup 0 gt?
] while drop
$_acc  # 100! is a 158-digit number
```

### Bitwise

| Operation | Description |
|-----------|-------------|
| `big-xor` | XOR |
| `big-and` | AND |
| `big-or` | OR |
| `big-shl` | Shift left |
| `big-shr` | Shift right |

**Examples:**

```hsab
# XOR two values (useful for checksums)
"255" to-bigint "128" to-bigint big-xor
# Returns: BigInt(127)

# Bit shifting
"1" to-bigint 64 big-shl
# Returns: BigInt(18446744073709551616) - 2^64

# Masking
"0xFFFFFFFF" from-hex to-bigint
"0x12345678" from-hex to-bigint
big-and
```

### Comparisons

| Operation | Description |
|-----------|-------------|
| `big-eq?` | Equal |
| `big-lt?` | Less than |
| `big-gt?` | Greater than |

**Examples:**

```hsab
# Compare large numbers
"999999999999999999999" to-bigint
"999999999999999999998" to-bigint
big-gt?  # Exit 0 (true)

# Check if value exceeds threshold
$value to-bigint
"1000000000000000000" to-bigint
big-lt? ["Under limit" echo] ["Over limit!" echo] if
```

---

## Module System

### Importing Modules

```hsab
"utils.hsab" .import            # Import as utils::
"utils.hsab" myutils .import    # Import with alias
```

### Using Namespaced Functions

```hsab
utils::helper                   # Call function from utils module
myutils::process                # Call with custom alias
```

### Private Definitions

```hsab
[internal-impl] :_helper        # Private (underscore prefix)
```

### Search Path

Modules are searched in order:
1. Current directory (`.`)
2. `./lib/`
3. `~/.hsab/lib/`
4. `$HSAB_PATH` directories

### Practical Examples

**Creating a reusable module:**

```hsab
# File: lib/math-utils.hsab

# Public: square a number
[dup *] :square

# Public: cube a number
[dup dup * *] :cube

# Private: internal helper (underscore prefix)
[2 /] :_half

# Public: uses private helper
[square _half] :half-square
```

**Using the module:**

```hsab
# Import with default namespace
"lib/math-utils.hsab" .import

5 math-utils::square     # Returns: 25
3 math-utils::cube       # Returns: 27

# Import with custom alias
"lib/math-utils.hsab" m .import
5 m::square              # Returns: 25
```

**Conditional module loading:**

```hsab
# Load different modules based on environment
$ENV "production" eq? [
  "prod-config.hsab" .import
] [
  "dev-config.hsab" .import
] if
```

---

## Plugin System

Extend hsab with WebAssembly (WASM) plugins for performance-critical operations.

### Loading Plugins

```hsab
"path/plugin.wasm" .plugin-load     # Load WASM plugin
"plugin-name" .plugin-unload        # Unload plugin
"plugin-name" .plugin-reload        # Force reload
```

### Plugin Management

```hsab
.plugins                            # List all loaded plugins
"plugin-name" .plugin-info          # Show plugin details
```

### Plugin Directory

Plugins are stored in `~/.hsab/plugins/` with TOML manifests.

### Practical Examples

**Loading a crypto plugin:**

```hsab
# Load a WASM plugin for fast cryptography
"~/.hsab/plugins/fast-crypto.wasm" .plugin-load

# Use plugin functions (they appear as builtins)
"secret" plugin-encrypt
"encrypted" plugin-decrypt
```

**Listing and managing plugins:**

```hsab
# See what's loaded
.plugins
# Output:
# Loaded plugins:
#   fast-crypto (v1.2.0) - Fast cryptographic operations
#   json-tools (v2.0.1) - JSON manipulation utilities

# Get details about a plugin
"fast-crypto" .plugin-info
# Output:
# Name: fast-crypto
# Version: 1.2.0
# Functions: encrypt, decrypt, hash, sign, verify

# Reload after updating
"fast-crypto" .plugin-reload
```

**Plugin manifest (TOML):**

```toml
# ~/.hsab/plugins/fast-crypto.toml
name = "fast-crypto"
version = "1.2.0"
wasm_file = "fast-crypto.wasm"
description = "Fast cryptographic operations"
```

---

## Meta Commands

Meta commands (dot-prefixed) affect shell state:

### Clipboard

```hsab
value .copy                 # Copy to clipboard
value .cut                  # Cut to clipboard (drop + copy)
.paste                      # Paste from clipboard
paste-here                  # Inline paste expansion
```

### Aliases

```hsab
[block] "name" .alias       # Define alias
name .unalias               # Remove alias
.alias                      # List all aliases
```

### Signal Traps

```hsab
[cleanup] SIGINT .trap      # Set signal handler
.trap                       # List all traps
```

---

## REPL Commands

| Command | Alias | Description |
|---------|-------|-------------|
| `.help` | `.h` | Show help |
| `.stack` | `.s` | Show stack |
| `.peek` | `.k` | Show top value |
| `.pop` | `.p` | Pop and show |
| `.clear` | `.c` | Clear stack and screen |
| `clear-stack` | | Clear stack only |
| `clear-screen` | | Clear screen only |
| `.use` | `.u` | Move top to input |
| `.use=N` | `.u=N` | Move N items to input |
| `.types` | `.t` | Toggle type annotations |
| `.hint` | | Toggle hint visibility |
| `.highlight` | `.hl` | Toggle syntax highlighting |
| `exit` | `quit` | Exit REPL |

### Syntax Highlighting

When enabled, the REPL colorizes input as you type:

| Token Type | Color | Example |
|------------|-------|---------|
| Builtins | Blue | `echo`, `dup`, `map` |
| Strings | Green | `"hello"`, `'text'` |
| Numbers | Yellow | `42`, `3.14` |
| Blocks | Magenta | `[echo hello]` |
| Operators | Cyan | `@`, `\|`, `:` |
| Variables | Cyan | `$HOME`, `$name` |
| Comments | Dim | `# comment` |
| Definitions | Bold | User-defined words |

Enable via environment variable or REPL command:

```bash
# Environment variable (in .bashrc or .zshrc)
export HSAB_HIGHLIGHT=1

# Or toggle at runtime
hsab> .highlight
Syntax highlighting: ON
```

See [Configuration Guide](config.md#syntax-highlighting) for details.

### Debugger

```hsab
.debug, .d                  # Toggle debug mode
.break <pat>, .b <pat>      # Set breakpoint
.delbreak <pat>, .db <pat>  # Remove breakpoint
.breakpoints, .bl           # List breakpoints
.clearbreaks, .cb           # Clear all breakpoints
.step                       # Enable single-step
```

When paused in debugger:
- `n` / `next` / Enter - Step to next
- `c` / `continue` - Continue to breakpoint
- `s` / `stack` - Show stack
- `b` / `breakpoints` - List breakpoints
- `q` / `quit` - Quit debug mode

### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| Alt+Up | Push first word to stack |
| Alt+Down | Pop to input |
| Alt+A | Push ALL words to stack |
| Alt+a | Pop ALL to input |
| Alt+k | Clear stack |
| Alt+c | Copy top to clipboard |
| Alt+x | Cut top to clipboard |
| Alt+t | Toggle type annotations |
| Alt+h | Toggle hint |
| Ctrl+O | Pop one (compatibility) |

---

## Examples

### Basic Usage

```hsab
# Simple command
hello echo                      # echo hello

# Arguments (LIFO order)
-la ls                          # ls -la

# Command substitution
pwd ls                          # ls $(pwd)

# Blocks and apply
[hello echo] @                  # Execute block

# Piping
ls [grep txt] |                 # ls | grep txt

# Definitions
[dup mul] :square
5 square                        # 25
```

### File Operations

```hsab
# Copy with extension change
file.txt dup .bak reext cp      # cp file.txt file.bak

# Define reusable backup
[dup .bak reext cp] :backup
file.txt backup

# Process all .txt files
"*.txt" glob spread [wc -l] each collect
```

### Structured Data

```hsab
# Parse JSON
'{"name":"Alice","age":30}' json
"name" get                      # Alice

# Work with tables
"data.csv" open
["age" 25 gt?] where
"name" sort-by
to-json
```

### Async Operations

```hsab
# Run tasks in parallel
[task1] async [task2] async
2 future-await-n                # Wait for both

# With timeout
5 [slow-operation] timeout
```

---

## Startup Files

| File | When Loaded |
|------|-------------|
| `~/.hsab/lib/stdlib.hsabrc` | Always (if exists) |
| `~/.hsabrc` | Interactive startup |
| `~/.hsab_profile` | Login shell (`-l`) |

Run `hsab init` to install the standard library.
