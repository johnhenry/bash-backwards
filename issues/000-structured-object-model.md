# 000 — Structured Object Model (Keystone)

**Priority:** Critical — this is hsab's strategic differentiator
**Scope:** Runtime type system, pipeline data flow, display formatting, external command boundary
**Depends on:** Nothing (foundational)
**Blocks:** 001–008 (all sub-issues)

---

## Summary

Add a structured data model to hsab so that values on the stack can be **records** (key-value maps), **tables** (lists of records), and **lists** (ordered sequences of typed values) — not just strings and blocks. This transforms hsab from "bash with a stack" into "a stack-based shell with native structured data," a combination no other shell offers.

---

## Strategic Motivation

hsab currently occupies an awkward middle ground:

- Too unconventional for users who just want a "better bash" (they'll pick fish or zsh)
- Not radical enough for users who want structured data (they'll pick Nushell)

The stack model solves real problems — holding intermediate results, reusing values, fan-out, higher-order operations — but these are narrow pain points. Structured data is the commercially valuable bet. **Combining the stack with native structured types is genuinely novel.** No existing shell does this.

### What hsab can do that Nushell, PowerShell, and Elvish cannot

1. **Operate on multiple tables simultaneously.** Nushell's pipeline is linear — one value flows through at a time. hsab's stack lets you push two tables, `swap` between them, `dup` one to operate on it while keeping the other, and `join-on` without needing variables or subexpressions.

2. **Accumulate results across operations.** Each API call or query pushes its result onto the stack. At the end, all results are available without intermediate variable names. This is a natural fit for report building, dashboard assembly, and multi-source data aggregation.

3. **Fan-out structured data.** `dup` a table and send one copy to a file, another to an API, and keep a third on the stack for further processing — all without temp variables or `tee` gymnastics.

4. **Pass structured transformations as values.** A block like `["size" get 1024 div]` is a first-class transformation that can be stored, composed, and applied to any table. This is genuinely more composable than Nushell's closures or PowerShell's scriptblocks because blocks live on the same stack as the data they transform.

---

## Prior Art: How Other Shells Handle This

### Nushell — Interior-first, typed pipeline

**Architecture:** Every command returns typed `Value` objects (17 variants). Data flows through `PipelineData` which has four variants: `Empty`, `Value` (single materialized value), `ListStream` (lazy iterator over values), and `ByteStream` (raw bytes from external commands). The `Value` enum is deliberately constrained to 56 bytes with boxing for larger types.

**Types:** Bool, Int, Float, String, Filesize, Duration, Date, Range, Binary, List, Record, Closure, Block, Nothing, Error, CellPath, Custom.

**Tables:** Internally represented as `List<Record>`. This is an important implementation detail — there's no special Table type, just a list of records that display as tables when they're uniform.

**External boundary:** When piping to external commands, Nushell serializes structured data through its table formatter — the same visual table you see at the terminal goes into the external command's stdin. This means external commands get beautiful but unparseable output by default. Users must explicitly use `to json`, `to csv`, etc. for machine-readable output. External command output returns as `ByteStream` and stays as raw text until explicitly parsed with `from json`, `from csv`, etc.

**Problems:**
- The table formatter going to external commands is a common complaint (issue #5617)
- No dual-channel approach — you're either in structured-land or text-land
- Large datasets hit memory issues because ListStream still materializes records
- Cell-path access (`$record.field?.nested`) is powerful but requires learning a DSL

**Lesson for hsab:** Nushell proves the concept works and users want it. Their mistakes are: auto-formatting structured data for external commands (lossy), no easy escape hatch to raw text mode, and memory pressure from eager materialization.

### PowerShell — Object pipeline with format-on-display

**Architecture:** .NET objects flow through the pipeline with full type information, methods, and properties. The key insight is **format-on-display**: at the end of every interactive pipeline, `Out-Default` is invisibly appended. It passes objects to the formatting system, which decides how to render them based on three rules:
1. Does the type have a predefined format view (`.format.ps1xml`)?
2. Does the type declare a "default display property set"?
3. Fallback: ≤4 properties → table, ≥5 → list.

**Critical design principle:** Intermediate pipeline stages never see rendered text. Objects stay rich. Only the terminal output triggers formatting. This means `Get-Process | Where-Object CPU -gt 10` operates on real Process objects with all their properties, not on a 4-column rendered table.

**External boundary:** When piping to external commands, PowerShell converts objects to their `.ToString()` representation, which is usually unhelpful (you get type names like `System.Diagnostics.Process`). Users must explicitly use `ConvertTo-Json`, `ConvertTo-Csv`, etc. This is arguably worse than Nushell's approach.

**Problems:**
- Verbose syntax (`Select-Object -Property @{Label='size_kb';Expression={$_.Size/1024}}`)
- .NET dependency makes it heavy and non-portable
- External command interop is clunky — piping to `grep` feels unnatural
- Heterogeneous pipelines (mixed types) produce messy output

**Lesson for hsab:** Format-on-display is the single most important idea to steal. Also: keep external interop frictionless (PowerShell's biggest Linux weakness). And: postfix syntax is naturally terser than PowerShell's verb-noun convention, which is an advantage.

### Elvish — Dual-channel pipeline

**Architecture:** Elvish runs two channels in parallel for every pipeline:
1. A traditional **byte channel** (stdout/stdin text)
2. An internal **value channel** (structured data: lists, maps, closures)

The `echo` command writes to the byte channel (serializes to string). The `put` command writes to the value channel (preserves structure). Both channels are active simultaneously.

**Types:** Strings, Lists (`[]`), Maps (`&key=value`), Functions (first-class closures), Numbers.

**External boundary:** External commands only see the byte channel. Internal commands can read from either. This is the cleanest separation of any shell — external tools don't even know structured data exists.

**Problems:**
- The dual-channel concept is confusing for newcomers
- No table type (maps of maps, but no uniform tabular representation)
- Smaller ecosystem and community than Nushell
- Error handling via exceptions can be surprising

**Lesson for hsab:** The dual-channel idea is elegant but may be too complex. A simpler approach: structured data auto-serializes to text at external boundaries (one channel, smart serialization). Elvish proves that keeping external commands on a text-only channel works well.

### YSH/Oils — Exterior-first

**Architecture:** YSH is "exterior-first" — its pipelines are real OS processes communicating over `pipe()`, with structured data layered on top via textual data languages (JSON, TSV, J8 Notation). This is the opposite of Nushell/PowerShell which are "interior-first" (native objects that must be serialized for external commands).

**Key insight:** Data languages (JSON, TSV) are the interchange format. Internal data structures map one-to-one to external data languages. This means you can always `cat` a file and get something a YSH script can natively consume, and YSH scripts naturally produce output other tools can consume.

**Lesson for hsab:** The exterior-first philosophy is the most compatible with Unix traditions. hsab should lean this direction — structured data is the internal representation, but serialized data languages (JSON, TSV, CSV) are always one step away. Never require special tooling to read hsab output.

---

## Proposed Type System for hsab

### Core types (Rust enums)

```rust
enum Value {
    // Primitives
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Nothing,

    // Existing
    Block(Block),

    // Structured (NEW)
    Record(IndexMap<String, Value>),
    List(Vec<Value>),

    // Special
    Error(ErrorRecord),
    Binary(Vec<u8>),
}
```

**Design decisions:**

- **No separate Table type.** A table is a `List<Record>` where all records share the same keys. This is simpler (one fewer type to handle everywhere) and matches Nushell's successful approach. The display system detects uniform lists-of-records and renders them as tables.

- **IndexMap for records.** Preserves insertion order (important for display and serialization) while providing O(1) key lookup. Use the `indexmap` crate.

- **No Filesize/Duration/Date types initially.** Start minimal. These can be added later as the stdlib matures. Strings with conventions (like `"1.5MB"`) work fine early on.

- **Error is a type, not a side channel.** Failed commands push an `ErrorRecord` onto the stack. This makes errors inspectable, filterable, and composable — you can `dup` an error, extract its message, retry the operation. See issue #005.

### Stack behavior

Every value pushed to the stack is a `Value`. Operations on the stack must be type-aware:

```
"hello" 5 plus    # ERROR: cannot add Str + Int
[1 2 3] list "name" get   # ERROR: List has no key "name"
{"name" "hsab"} record "name" get   # OK: "hsab"
```

Type errors should be clear and immediate, not deferred. hsab should fail loudly when types don't match — this is a major advantage over bash where everything is a string and errors are silent.

---

## Syntax for Structured Data

### Records

```
# Literal syntax: alternating key-value pairs + record keyword
{"name" "hsab" "version" "0.2.0" "lang" "rust"} record

# Or: implicit from braces (if unambiguous)
{name: "hsab", version: "0.2.0", lang: "rust"}

# Recommendation: use the first form (stack-consistent, no new syntax)
# The key-value pair order is: key value key value ...
```

**Rationale for first form:** It's consistent with hsab's philosophy — values are pushed onto a stack, `record` consumes them pairwise. No new syntax required. The colon/comma form is sugar that can come later.

### Lists

```
# Literal syntax
[1 2 3 4 5] list
["alpha" "beta" "gamma"] list

# Or: collect from stack operations
1 2 3 4 5 5 collect   # collects 5 items into a list
```

### Tables (uniform list of records)

```
# Built from individual records
[
  {"name" "foo.rs" "size" 1024} record
  {"name" "bar.rs" "size" 2048} record
  {"name" "baz.rs" "size" 512} record
] list

# Or: from builtins that return structured data
ls              # returns List<Record> with name, type, size, modified
ps              # returns List<Record> with pid, name, cpu, mem
```

### Column/field access

```
# Single field from a record
"name" get              # pops record, pushes value of "name" field

# Column from a table (applies get to each record)
"name" get              # if top-of-stack is List<Record>, extracts column as List<Str>

# Nested access with dot-path
"server.host" get       # navigates nested records

# Setting values
"version" "0.3.0" set   # pops record, pushes new record with updated field
```

**Key design choice:** `get` is polymorphic. On a Record, it extracts a field. On a Table (List<Record>), it extracts a column (maps `get` over each record). This means the same syntax works for both single records and tables, which is powerful and consistent.

### Table operations

```
# Filter rows
ls ["size" get 1024 gt?] keep

# Sort
ls "size" sort-by
ls "name" sort-by-desc

# Group
ls "type" group-by           # Record<type -> List<Record>>

# Aggregate
ls "size" get sum
ls "size" get avg
ls "size" get min
ls "size" get max
ls count                     # number of rows

# Select columns
ls ["name" "size"] select

# Add computed column
ls ["size" get 1024 div] "size_kb" add-column

# Transform column in place
ls "size" [1024 div] map-column

# Rename
ls "size" "bytes" rename-column

# Join two tables
users orders "user_id" "id" join-on

# Unique/deduplicate
ls "type" get unique

# First/last N
ls 5 first
ls 3 last

# Transpose
ls transpose           # columns become rows
```

---

## Format-on-Display

**This is the highest-priority PowerShell idea to adopt.**

### Principle

Structured data flows through the pipeline as rich typed values. Only at the terminal — when a value would be printed to the user — does the display formatter kick in. Intermediate pipeline stages never see rendered text.

### How it works

1. When a command completes and the stack has values that would be displayed to the user, the **display formatter** runs.
2. The formatter inspects the type:
   - `List<Record>` with uniform keys → render as table
   - `Record` → render as key-value list
   - `List<Str>` or `List<Int>` → render as newline-separated values
   - `Str` → render as-is
   - `Error` → render in red with message, source, and code
3. The formatter **never modifies the stack value.** The rich data remains on the stack for subsequent operations.

### Display formatting rules

```
# Table display (List<Record> with uniform keys)
┌────────────┬──────┬───────┬─────────────────────┐
│ name       │ type │ size  │ modified            │
├────────────┼──────┼───────┼─────────────────────┤
│ Cargo.toml │ file │ 1234  │ 2025-01-15 10:30   │
│ src/       │ dir  │ 4096  │ 2025-01-15 11:00   │
└────────────┴──────┴───────┴─────────────────────┘

# Record display (single record)
name:     hsab
version:  0.2.0
lang:     rust

# List display (flat list)
alpha
beta
gamma

# Nested structure display
server:
  host: localhost
  port: 8080
logging:
  level: debug
  file: /var/log/app.log
```

### User control over display

```
ls to-table          # force table display
ls to-list           # force list display
ls to-json           # JSON format
ls to-csv            # CSV format
ls to-tsv            # TSV format
ls to-yaml           # YAML format (if supported)
```

### REPL stack display

The `.s` (show stack) command should show types and abbreviated previews:

```
hsab¢ .s
| Table(3 rows × 4 cols) Record{name,version} "hello"(str) 42(int) |
```

---

## External Command Interop

**This is the make-or-break design decision.** Get this wrong and hsab becomes another PowerShell-on-Linux — technically superior but practically unusable.

### Core principle: Asymmetric serialization

```
structured → external:  AUTO-SERIALIZE (always works, user doesn't think about it)
external → structured:  EXPLICIT PARSE  (user decides when to upgrade text to typed data)
```

This asymmetry is intentional and critical:

- **Auto-serialization** is mechanical. Given a table, producing TSV is deterministic. There's one right answer.
- **Parsing** requires format knowledge. Is this CSV? TSV? JSON? Fixed-width columns? The user knows; the shell doesn't.

### Outbound: structured → external

When a structured value is piped to an external command, hsab auto-serializes:

| Type | Default serialization |
|------|----------------------|
| `List<Record>` (table) | TSV (tab-separated, header row) |
| `List<Str>` | Newline-separated |
| `List<Int/Float>` | Newline-separated |
| `Record` | `key=value` lines |
| `Str` | As-is |
| `Int/Float` | String representation |
| `Nothing` | Empty string |

```
# These just work:
ls "name" get [grep "test"] |       # List<Str> → newline-separated → grep
ls [awk '{print $1}'] |             # Table → TSV → awk sees columns
ls to-json [jq '.[] | .name'] |     # Explicit JSON for jq

# User can override:
ls to-csv [some-csv-tool] |         # CSV instead of TSV
ls to-json [curl -d @- url] |       # JSON for API calls
ls to-ndjson [external-tool] |      # Newline-delimited JSON
```

**Why TSV as default table format:** TSV is the most Unix-native tabular format. `awk`, `cut`, `sort`, `join` all work with tab-separated data by default. CSV has quoting rules that break `awk`. JSON is verbose and requires `jq`. TSV is the path of least resistance.

### Inbound: external → structured

External command output is **always text** until the user explicitly parses it:

```
# Raw text (default)
curl -s https://api.github.com/users/john     # → Str on stack

# Explicit parse
curl -s https://api.github.com/users/john json    # → Record on stack
cat data.csv into csv                              # → Table on stack
ps aux into tsv                                    # → Table on stack
env into kv "="                                    # → Record on stack
df -h into columns                                 # → Table on stack
```

**Why never auto-parse:**
1. Auto-parsing requires heuristics that will be wrong
2. Nushell's issue #5617 shows the pain of wrong auto-detection
3. Explicit parsing makes scripts predictable and debuggable
4. Text is a perfectly valid result — not everything needs to be a table

### The `into` bridge operator

`into` is the explicit boundary crossing from text to structured data:

```
into json       # parse JSON → Record or List
into csv        # parse CSV → Table (first row = headers)
into tsv        # parse TSV → Table (first row = headers)
into kv "="     # parse key=value lines → Record (custom delimiter)
into columns    # parse fixed-width columns → Table (detect by whitespace alignment)
into lines      # split text into List<Str> by newlines
into words      # split text into List<Str> by whitespace
into ndjson     # parse newline-delimited JSON → List<Record>
```

### The `raw` escape hatch

Sometimes you want to force text mode even for builtins that would normally return structured data:

```
raw ls                    # returns text output of ls, not structured table
raw [ls [grep foo] |]     # entire block runs in text mode
```

This is important for backward compatibility and for cases where the structured output is wrong or unwanted.

### Mixed workflows (the real test)

```
# git log → structured → filtered → text → external
git log --format="%H|%an|%s" into csv "|"
["author" get "john" eq?] keep
"subject" get
[head -5] |

# docker → structured → filtered → action
docker ps --format '{{json .}}' into ndjson
["Status" get "Up" starts?] keep
"Names" get
[xargs docker stop] |

# Multi-source join (hsab's unique advantage)
curl -s api.example.com/users json             # table 1 on stack
curl -s api.example.com/orders json            # table 2 on stack
"user_id" "id" join-on                         # join on stack
["status" get "active" eq?] keep
"total" get sum

# CSV → transform → JSON
"sales.csv" open into csv
"region" group-by ["amount" get sum] each
to-json "summary.json" write
```

---

## Implementation Plan

### Phase 1: Core types (MVP)

1. Add `Record`, `List`, `Error` variants to the `Value` enum
2. Implement `record` keyword to construct records from stack pairs
3. Implement `list` keyword to construct lists from stack values
4. Implement `get`, `set`, `keys`, `values` for records
5. Make `get` polymorphic (Record → field, List<Record> → column)
6. Implement display formatter (table/list/record detection)
7. Add `json` keyword to parse JSON strings into Record/List

### Phase 2: Table operations

1. `keep` (filter) with record field access
2. `sort-by`, `sort-by-desc`
3. `group-by`
4. Aggregates: `sum`, `avg`, `min`, `max`, `count`
5. `select` (column subset)
6. `add-column`, `map-column`, `rename-column`
7. `first`, `last`, `unique`
8. `join-on`

### Phase 3: External interop

1. Auto-serialization rules (table→TSV, list→newlines, record→kv)
2. `into` operator with `json`, `csv`, `tsv`, `kv`, `columns`, `lines`, `ndjson`
3. `to-json`, `to-csv`, `to-tsv`, `to-yaml` explicit serializers
4. `raw` escape hatch
5. `open` builtin (auto-detect file format by extension)

### Phase 4: Builtins return structured data

1. `ls` → `List<Record{name, type, size, modified}>`
2. `ps` → `List<Record{pid, name, cpu, mem, status}>`
3. `env` → `Record` of environment variables
4. `which` → `Record{name, path, type}`
5. `history` → `List<Record{index, command, timestamp}>`

---

## Open Questions

1. **Should `get` on a missing key push `Nothing` or error?** Recommendation: error by default, with `get?` (optional get) pushing `Nothing`. This matches Nushell's cell-path optional chaining.

2. **How large can tables get before memory is a problem?** Nushell uses lazy `ListStream` for large results. hsab should consider a similar lazy approach for builtins like `find` that can produce millions of results. But for MVP, eager lists are fine.

3. **Should records be immutable?** Recommendation: yes. `set` returns a new record. Immutability simplifies the stack model (no aliasing bugs) and matches functional shell patterns. Deep nested updates via `update-in` can come later.

4. **How does `spread` interact with tables?** `spread` currently pushes each stack item individually. For tables, `spread` should push each row (record) individually onto the stack. This enables `ls spread ["name" get echo] each` as an alternative to column extraction.

5. **What about schemas?** Not for MVP. But eventually, definitions could declare expected record shapes: `[{name: str, size: int} schema] :file-record`. This enables validation and better error messages.

---

## Success Criteria

- `ls ["size" get 1000 gt?] keep "name" get` works end-to-end
- `curl -s api json ["field" get] each` works for JSON APIs
- `ls [grep test] |` auto-serializes to TSV for grep
- Two tables on the stack can be joined without variables
- `.s` shows meaningful type previews in the REPL
- Error from a failed command is inspectable with `"message" get`
- Existing hsab scripts that don't use structured data still work unchanged
