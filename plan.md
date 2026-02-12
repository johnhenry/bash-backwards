# hsab Object Model Integration Plan

## Thesis

hsab's stack-based data flow is genuinely novel among shells. Nushell proved structured data is the right bet. PowerShell proved format-on-display and error objects matter. hsab can combine all three — stack-based non-linear data flow with typed structured values and clean external tool interop — into something no existing shell offers.

This plan is ordered by implementation phase. Each phase is independently useful and ships incrementally.

---

## Phase 0: Internal Value Representation

Before any user-facing features, the interpreter needs a tagged value system. Every item on the stack gets a type tag.

### Value Types

```
String    — current default, all existing behavior preserved
Integer   — numeric (parsed lazily or explicitly)
Float     — numeric with decimal
Boolean   — true / false
Record    — ordered key-value map (keys are strings, values are any type)
Table     — list of records with consistent columns
List      — ordered collection of any values
Null      — explicit absence
Error     — structured error (see Phase 3)
Block     — existing block type, unchanged
```

### Key Constraint

Every value must serialize to text losslessly. This is the escape hatch that keeps external tool interop working. A Record serializes to `key=value\n` pairs, a Table to TSV, a List to newline-separated items. This is not optional — it's what prevents the Nushell trap of breaking `grep`.

### Implementation Notes

- Existing string-on-stack behavior is type `String` — zero breaking changes
- Type tags are internal metadata; untyped usage continues to work
- `typeof` operator exposes the tag: `42 typeof` → pushes `"Integer"` onto stack
- Comparisons and arithmetic auto-coerce where unambiguous (string "42" + 1 = 43), error otherwise

---

## Phase 1: Records

Records are the atomic structured type. Everything else builds on them.

### Construction

```
# Literal syntax — pairs of key value, terminated by `record`
"name" "hsab" "version" "0.2.0" "lang" "rust" record

# From key=value text (bridge from external tools)
"name=hsab\nversion=0.2.0" into kv

# Empty record
record   # with nothing before it, or after a marker
```

### Field Access

```
# Get a field — pushes the value
"name" get             # → "hsab"

# Nested access via dot-path
"server.port" get      # sugar for "server" get "port" get

# Set / update a field — returns new record (immutable)
"version" "0.3.0" set  # → record with version updated

# Remove a field
"lang" del             # → record without lang key

# Check existence
"name" has?            # → true/false

# Get all keys / values as lists
keys                   # → List ["name", "version", "lang"]
values                 # → List ["hsab", "0.2.0", "rust"]
```

### Stack Interaction (This Is What Nushell Can't Do)

```
# Two records on stack — compare without variables
"name" get    # gets name from top record
swap
"name" get    # gets name from second record
eq?           # compares them

# Merge two records (top overwrites conflicts)
merge         # pops two records, pushes merged result

# Duplicate and diverge
dup
"version" get     # extract version from copy
swap
"name" get        # extract name from original
# now both values on stack for further use
```

### Display (Format-on-Display, Stolen from PowerShell)

When a record hits the terminal (i.e., the implicit print at end of pipeline or explicit `show`), the display formatter renders it:

```
┌─────────┬───────┐
│ name    │ hsab  │
│ version │ 0.2.0 │
│ lang    │ rust  │
└─────────┴───────┘
```

Critically: this rendering happens **only at display time**. The record flowing through the pipeline is always the full structured object. Piping to another command passes the record, not the rendered text. This is the single most important lesson from PowerShell.

If the next command in the pipeline is an external binary, *then* the serializer fires and converts to text (see Phase 4).

---

## Phase 2: Tables and Lists

### Lists

```
# Literal — values between markers collected
| 1 2 3 | list          # → List [1, 2, 3]

# From newline-separated text
"alpha\nbeta\ngamma" into lines

# Operations
0 nth                   # → first element
length                  # → 3
[dup *] map             # → [1, 4, 9]  (square each)
[3 gt?] filter          # → [4, 9]
reverse
sort
flatten                 # nested lists → flat
unique                  # deduplicate
```

### Tables

A table is a list of records with consistent column names.

```
# From list of records
| {"name" "alice" "age" 30 record}
  {"name" "bob"   "age" 25 record}
  {"name" "carol" "age" 35 record} | table

# From CSV/TSV text (bridge from external tools)
"name,age\nalice,30\nbob,25" into csv

# From JSON array
'[{"name":"alice"},{"name":"bob"}]' into json

# From command output (the big one — see Phase 5)
ls    # built-in returns Table with name, size, type, modified columns
ps    # built-in returns Table with pid, name, cpu, mem columns
```

### Column Operations

```
# Select columns (like SQL SELECT)
| "name" "age" | select

# Get single column as list
"name" get               # → List ["alice", "bob", "carol"]

# Add computed column
"senior" ["age" get 30 gte?] add-column

# Rename column
"name" "username" rename-column

# Drop columns
| "age" | drop-columns

# Transform column values in place
"age" [1 +] map-column   # increment all ages
```

### Row Operations

```
# Filter rows (like SQL WHERE)
["age" get 30 gt?] where       # rows where age > 30

# Sort
"age" sort-by                   # ascending by age
"age" sort-by reverse           # descending

# First / last / nth
5 first                         # first 5 rows
3 last                          # last 3 rows
0 nth                           # single row (as record)

# Group
"department" group-by           # → Record of { dept_name: Table }

# Aggregate
"age" [sum] aggregate           # → single value
"age" [mean] aggregate
"name" [length] aggregate       # count

# Unique rows
unique

# Take while / skip while
["age" get 30 lt?] take-while
```

### Stack Interaction with Tables

This is hsab's differentiator. Multiple tables on the stack at once, operated on without naming them.

```
# Load two datasets
ls ~/projects
ls ~/archive

# Get names from both, compare
"name" get       # names from ~/archive (top of stack)
swap
"name" get       # names from ~/projects
# two lists on stack — diff, intersect, etc.
diff             # items in projects not in archive

# Join two tables (like SQL JOIN)
ls ~/src
open "metadata.json" into json
"name" inner-join   # join on shared "name" column

# Append tables vertically (like UNION)
append              # pops two tables, pushes combined
```

### Display

Tables render as bordered tables at display time:

```
┌───────┬─────┬────────┐
│ name  │ age │ senior │
├───────┼─────┼────────┤
│ alice │  30 │ true   │
│ bob   │  25 │ false  │
│ carol │  35 │ true   │
└───────┴─────┴────────┘
(3 rows)
```

Large tables auto-paginate or truncate with `... and 847 more rows`. The full data is always on the stack.

---

## Phase 3: Structured Errors

Errors become records on the stack instead of lost exit codes.

### Error Shape

```
{
  "kind"    : "command_failed"
  "message" : "ls: cannot access '/nope': No such file or directory"
  "command" : "ls"
  "code"    : 2
  "source"  : "/nope"
}
```

### Error Handling

```
# Errors propagate by default (like set -e)
# but they're values — you can catch them

ls /nope                   # pushes Error record onto stack
error?                     # → true (predicate, doesn't consume)
                           # pipeline halts unless handled

# Try/catch pattern
[ls /nope] try             # runs block, pushes result OR error
error? [
  "message" get            # extract message from error
  "Fallback: " swap cat    # build fallback string
] [
  # success path — result is on stack
] if-else

# Ignore errors explicitly
[ls /nope] try drop        # swallow the error

# Retry pattern
[fetch "https://api.example.com/data"] 3 retry
```

### Why This Matters

Bash error handling is `command || handle_error` with no structured info about *what* failed. `$?` is a single integer. hsab errors carry the command name, stderr content, exit code, and the input that triggered the failure — all as inspectable record fields.

```
[curl "https://down.example.com"] try
dup error? [
  "code" get 28 eq? [
    # timeout — retry with longer deadline
    drop
    curl --max-time 30 "https://down.example.com"
  ] when
] when
```

---

## Phase 4: External Command Interop

This is the make-or-break section. Nushell got this wrong (too aggressive about auto-parsing, breaks when external tools produce unexpected output). PowerShell got it wrong differently (everything is .NET objects, crossing to native tools is painful). hsab needs a third path.

### Core Principle: Asymmetric Boundary

```
STRUCTURED → EXTERNAL:  auto-serialize (always works, user never thinks about it)
EXTERNAL → STRUCTURED:  explicit parse (user opts in, never surprised)
```

### Outbound: Structured to External

When a structured value is piped to an external command (any binary not built into hsab), it auto-serializes:

| Type | Serialization |
|------|---------------|
| String | as-is |
| Integer/Float | decimal string |
| Boolean | "true" / "false" |
| Record | `key=value` lines (or single-line `key=value` pairs) |
| Table | TSV (header row + data rows) |
| List | newline-separated items |
| Null | empty string |
| Error | stderr message string |

```
# This always works — no user effort
ls | grep "\.rs$"
# ls returns Table, grep receives TSV text, filters lines, returns text

# Explicit format override when you need CSV or JSON
ls | to-csv | some-tool-that-wants-csv
ls | to-json | curl -d @- https://api.example.com
ls | to-ndjson | external-stream-processor
```

The user never needs to think about serialization when piping to external tools. It Just Works.

### Inbound: External to Structured

Output from external commands is **always a String**. Period. No guessing, no auto-detection, no "oh that looks like JSON."

```
# curl returns a String, even if it's JSON-shaped
curl -s "https://api.example.com/users"
typeof    # → "String"

# Explicit upgrade to structured
curl -s "https://api.example.com/users" into json
typeof    # → "Table" (if it was a JSON array) or "Record" (if object)

# Now you can do structured operations
"name" get
["active" get] where
```

### The `into` Bridge Operator

`into` is the explicit boundary crossing from text to structure. It's a family of commands:

```
into json       # parse JSON → Record or Table
into csv        # parse CSV → Table (first row = headers)
into tsv        # parse TSV → Table
into lines      # split on newlines → List
into words      # split on whitespace → List
into kv         # parse key=value pairs → Record
into columns    # split on whitespace, auto-detect columns → Table
into xml        # parse XML → nested Records
into toml       # parse TOML → Record
into yaml       # parse YAML → Record or Table
into regex "pattern"  # capture groups → Record or Table
```

### The Reverse: `to-*` Serializers

When you need a specific format (not the auto-serialize default):

```
to-json         # Record/Table → JSON string
to-csv          # Table → CSV string
to-tsv          # Table → TSV string
to-yaml         # Record/Table → YAML string
to-toml         # Record → TOML string
to-ndjson       # Table → newline-delimited JSON (one object per line)
to-kv           # Record → key=value lines
to-lines        # List → newline-separated string
to-md           # Table → Markdown table string
```

### Mixed Workflows

Real scripts mix external and structured operations freely:

```
# Fetch JSON API, filter, pass to external tool
curl -s "https://api.example.com/users" into json
["role" get "admin" eq?] where
"email" get
to-lines
| xargs -I{} send-notification {}

# Process log file with external tools, then structure the result
cat /var/log/app.log | grep "ERROR" | into columns
"timestamp" sort-by
["message" get "timeout" contains?] where
length
# → count of timeout errors

# Multiple external sources combined structurally
curl -s "https://api-a.com/data" into json
curl -s "https://api-b.com/data" into json
"id" inner-join
["status" get "active" eq?] where
to-csv > combined-report.csv
```

### Handling Ambiguity

What if the user pipes structured data to an external command that actually understands JSON?

```
# Default: auto-serialize to TSV (works with grep, awk, sort, etc.)
ls | grep "pattern"

# Explicit: serialize to JSON for tools that want it
ls | to-json | jq '.[] | .name'

# Hint: mark an external command as JSON-aware (definition)
:jq-wrapper [to-json | jq] def
```

The default is always the safe choice (text). Users opt into specific formats.

### External Command Detection

How does hsab know whether the next pipeline stage is a built-in or external command?

1. Built-in commands (ls, ps, cd, get, where, sort-by, etc.) receive structured values directly
2. Defined commands (user `:name [block] def`) receive structured values directly
3. Everything else is treated as an external binary — auto-serialize fires

This is a lookup at pipeline construction time, not a runtime guess.

---

## Phase 5: Structured Built-in Commands

Replace text-output built-ins with structured equivalents. The text-mode fallback means existing scripts don't break.

### File System

```
ls                  # → Table: name, type, size, modified, permissions
ls -l               # same table, "long" just changes the display formatter
"*.rs" glob         # → Table: name, path, size, modified
du                  # → Table: path, size
df                  # → Table: filesystem, size, used, available, mount

# These are still text when piped to external commands:
ls | grep ".rs"     # works — auto-serializes to TSV
```

### Process Management

```
ps                  # → Table: pid, name, cpu, mem, user, started
ps | ["cpu" get 50 gt?] where   # high CPU processes
```

### Networking

```
fetch "https://..."                 # → String (raw body)
fetch "https://..." --json          # → Record/Table (auto-parse JSON response)
fetch "https://..." --headers       # → Record (response headers)

open "file.json"                    # → Record/Table (auto-detect by extension)
open "file.csv"                     # → Table
open "file.toml"                    # → Record
open "file.yaml"                    # → Record/Table
```

`open` is the only place auto-parsing happens, and it's based on file extension — not content sniffing. This is predictable and explicit.

### System Info

```
env                 # → Record of all environment variables
"PATH" env-get      # → String
"MY_VAR" "value" env-set

hostname            # → String (some things are just strings, that's fine)
which "python"      # → Record: path, version, type (alias/binary/builtin)
```

---

## Phase 6: Advanced Operations

Once the basics are solid, add operations that exploit the stack + structure combination.

### Computed Joins

```
# Inner join on matching column
"id" inner-join          # pops two tables, pushes joined table

# Left join (keep all rows from first table)
"id" left-join

# Cross join (cartesian product)
cross-join

# Custom join predicate
["name" get swap "username" get eq?] join-where
```

### Pivot / Reshape

```
# Wide → long
| "date" | unpivot          # keep date, unpivot other columns into name/value

# Long → wide
"category" "value" pivot    # category values become column names
```

### Window Operations

```
# Running total
"amount" [sum] rolling 7    # 7-row rolling sum

# Rank
"score" rank                # add rank column by score
```

### Providers (Exploratory — Phase 6b)

Uniform navigation across different stores, inspired by PowerShell:

```
fs:/var/log ls              # filesystem (default, so "ls /var/log" works)
env: ls                     # environment variables as table
docker: ls                  # running containers as table
k8s:pods ls                 # kubernetes pods as table
git:log ls                  # git log as table
```

Each provider implements a standard interface: `ls`, `get`, `set`, `del`. The structured data model means the output is always a Table or Record regardless of the underlying store.

This is ambitious and should only be attempted after Phases 0–5 are stable.

---

## Phase 7: REPL Enhancements for Structured Data

The interactive experience needs to support the new types.

### Stack Preview

```
hsab> ls ~/projects
[0] Table (12 rows × 5 cols: name, type, size, modified, permissions)

hsab> "name" get
[0] List ["project-a", "project-b", ...]

hsab> dup
[0] List ["project-a", "project-b", ...]
[1] List ["project-a", "project-b", ...]
```

Stack items show type summaries. `peek` or `inspect` expands them:

```
hsab> peek
┌────────────┬──────┬────────┬─────────────┬───────┐
│ name       │ type │ size   │ modified    │ perms │
├────────────┼──────┼────────┼─────────────┼───────┤
│ project-a  │ dir  │ 4096   │ 2025-01-15  │ 755   │
│ project-b  │ dir  │ 4096   │ 2025-02-01  │ 755   │
│ ...        │      │        │             │       │
└────────────┴──────┴────────┴─────────────┴───────┘
```

### Tab Completion

When the top of stack is a Record or Table, tab-complete field names after `get`, `set`, `sort-by`, `where`:

```
hsab> ls ~/projects
hsab> "na<TAB>
→ "name"

hsab> sort-by "<TAB>
→ name  type  size  modified  permissions
```

### --trace Mode

Show the stack state and types after each operation:

```
hsab> --trace ls ~/projects | "name" get | [".rs" ends-with?] filter
  ls ~/projects          → [Table(12×5)]
  "name" get             → [List(12)]
  [".rs" ends-with?] filter → [List(3)]
```

---

## Implementation Order and Dependencies

```
Phase 0: Value tags                    — no user-facing changes, enables everything
  ↓
Phase 1: Records + get/set/merge       — first visible structured type
  ↓
Phase 2: Tables + Lists + column ops   — structured data becomes useful
  ↓
Phase 3: Error records                 — reliability baseline
  ↓
Phase 4: into/to-* interop bridge      — external tools work cleanly
  ↓
Phase 5: Structured built-ins (ls/ps)  — the "aha moment" for new users
  ↓
Phase 6: Joins, pivots, providers      — power features
  ↓
Phase 7: REPL enhancements             — polish
```

Phases 0–4 are the critical path. A shell with Records and Tables but no interop bridge is useless. A shell with the interop bridge but no structured built-ins is merely promising. Phase 5 is where hsab becomes *compelling*.

Phases 6–7 can be done in any order and are driven by user demand.

---

## What This Achieves

When complete, hsab occupies a position no existing shell holds:

| Capability | bash | Nushell | PowerShell | hsab |
|---|---|---|---|---|
| Structured data in pipeline | ✗ | ✓ | ✓ | ✓ |
| Non-linear data flow (stack) | ✗ | ✗ | ✗ | ✓ |
| External tool interop | ✓ | Fragile | Painful | ✓ (asymmetric bridge) |
| Format-on-display | ✗ | Partial | ✓ | ✓ |
| Multiple datasets in flight | ✗ (temp vars) | ✗ (linear pipe) | ✗ (variables) | ✓ (stack) |
| Blocks as values | ✗ | ✗ | ScriptBlock | ✓ |
| Structured errors | ✗ ($?) | ✓ | ✓ | ✓ |
| Gradual adoption from text | N/A | Poor | Poor | ✓ (into bridge) |

The unique selling point becomes: **"A shell where you can have two API responses on the stack, join them by user ID, filter the result, and pipe it to `jq` — all without naming a single variable or losing data to text serialization."**