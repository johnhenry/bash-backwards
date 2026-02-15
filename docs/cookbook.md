# hsab Cookbook

A collection of practical recipes and one-liners for hsab, the postfix notation shell.

---

## 1. File Operations

### Count Lines in Files

**Problem:** Count the number of lines in each `.rs` file.

```bash
*.rs ls spread [wc -l] each
```

**How it works:**
- `*.rs ls` runs `ls` with the glob, producing file list
- `spread` splits the output onto the stack (with marker)
- `[wc -l] each` runs `wc -l` on each file

**Variant:** Total line count across all files:

```bash
*.rs ls spread [wc -l] each collect into-lines sum
```

---

### Find Large Files

**Problem:** Find files larger than 1MB in the current directory.

```bash
. ls-table [["size" get 1048576 gt?]] where
```

**How it works:**
- `ls-table` returns a structured table with name, type, size, modified columns
- `where` filters rows where the predicate block returns exit code 0
- `"size" get` extracts the size field from each row record
- `1048576 gt?` checks if greater than 1MB (1024 * 1024 bytes)

**Sort by size, largest first:**

```bash
. ls-table "size" sort-by reverse
```

---

### Batch Rename Files

**Problem:** Rename all `.txt` files to `.md`.

```bash
*.txt ls spread [dup .md reext] each [mv] each
```

**How it works:**
- `*.txt ls spread` gets all txt files onto stack
- `[dup .md reext] each` creates pairs: `old.txt new.md old.txt new.md ...`
- `[mv] each` runs mv on each pair

**Preview before executing:**

```bash
*.txt ls spread [dup .md reext] each .s
# Inspect the pairs, then:
[mv] each
```

---

### Search and Replace in Files

**Problem:** Replace "foo" with "bar" in all Python files.

```bash
*.py ls spread ["s/foo/bar/g" -i sed] each
```

**Alternative using hsab's string operations:**

```bash
[
  _FILE local
  $_FILE cat "foo" "bar" str-replace $_FILE >
] :replace-foo-bar

*.py ls spread [replace-foo-bar] each
```

---

### Find Files by Pattern and Process

**Problem:** Find all test files and run them.

```bash
. -name "*_test.go" find spread [go test] each
```

**Recursively find and count:**

```bash
. -name "*.rs" -type f find spread depth
```

---

## 2. Text Processing

### Extract Columns

**Problem:** Extract the second column from space-separated data.

```bash
data.txt cat into-lines [[" " split1 swap drop " " split1 drop] map]
```

**Using tables for CSV data:**

```bash
data.csv open "column_name" get
```

**AWK-style column extraction:**

```bash
data.txt cat spread [" " split1 swap drop " " split1 drop] each collect
```

---

### Filter Lines

**Problem:** Keep only lines containing "ERROR".

```bash
app.log cat into-lines [["ERROR" contains?] filter]
```

**Using shell grep with hsab processing:**

```bash
app.log ERROR grep spread unique depth
# Count unique error lines
```

**Keep lines NOT matching a pattern:**

```bash
app.log cat into-lines [["DEBUG" contains?] reject]
```

---

### Transform Data

**Problem:** Convert a list of names to uppercase.

```bash
'["alice", "bob", "charlie"]' into-json [[upper] map]
```

**Add prefix to each line:**

```bash
data.txt cat spread ["PREFIX: " swap suffix] each collect
```

**Number each line:**

```bash
data.txt cat into-lines
1 _N local
[[dup $_N swap suffix $_N 1 plus _N local] map]
```

---

### CSV Processing

**Problem:** Read a CSV, filter rows, and write back.

```bash
# Read CSV into table
data.csv open

# Filter rows where status is "active"
[["status" get "active" eq?]] where

# Select specific columns
["name" "email"] select

# Save as new CSV
"filtered.csv" save
```

**Aggregate CSV data:**

```bash
sales.csv open "amount" get sum
```

**Group and analyze:**

```bash
data.csv open "category" group-by
# Result: Record where each key maps to a sub-table
```

---

## 3. JSON Manipulation

### Parse and Query JSON

**Problem:** Extract a nested value from JSON.

```bash
'{"user": {"name": "Alice", "age": 30}}' into-json "user.name" get
# Result: "Alice"
```

**Query arrays:**

```bash
'{"items": [1, 2, 3, 4, 5]}' into-json "items" get sum
# Result: 15
```

---

### Transform JSON Structures

**Problem:** Add a field to each object in an array.

```bash
'[{"name": "Alice"}, {"name": "Bob"}]' into-json
[[dup "name" get "processed_" swap suffix "id" swap set] map]
```

**Flatten nested structure:**

```bash
'[[1, 2], [3, 4], [5]]' into-json flatten
# Result: [1, 2, 3, 4, 5]
```

---

### Merge JSON Files

**Problem:** Merge two JSON configuration files.

```bash
config.json open defaults.json open merge "merged.json" save
```

**Deep merge (right overwrites left):**

```bash
base.json open overrides.json open merge
```

---

### API Response Processing

**Problem:** Extract data from an API response.

```bash
"https://api.example.com/users" fetch "data" get
[[["name" "email"] fields record] map]
```

**Handle pagination:**

```bash
[
  1 _PAGE local
  marker
  [true] [
    "https://api.example.com/users?page=$_PAGE" fetch
    dup "data" get spread
    "next" get nil? [break] @ if
    $_PAGE 1 plus _PAGE local
  ] while
  collect
] :fetch-all-pages
```

---

## 4. System Administration

### Disk Usage

**Problem:** Find the largest directories.

```bash
. -maxdepth 1 du -sh spread ["	" split1 drop] each
```

**Structured disk analysis:**

```bash
. ls-table [[type "dir" eq?]] where "size" sort-by reverse 10 first
```

---

### Process Management

**Problem:** Find and kill processes by name.

```bash
# List processes matching a pattern
python pgrep spread

# Kill all matching processes (careful!)
python pgrep spread [kill] each
```

**Monitor a process:**

```bash
[
  _PID local
  [true] [
    $_PID ps -p
    1 sleep
  ] while
] :watch-pid
```

---

### Log Analysis

**Problem:** Analyze error frequency in logs.

```bash
# Count errors by type
/var/log/app.log ERROR grep spread
[" ERROR " rsplit1 swap drop] each
unique depth
```

**Extract timestamps from errors:**

```bash
app.log cat into-lines
[["ERROR" contains?] filter]
[["T" split1 drop " " split1 swap drop] map]
unique
```

**Find most recent errors:**

```bash
/var/log/app.log ERROR grep spread 10 last collect
```

---

### Backup Script

**Problem:** Create timestamped backups.

```bash
[
  _SRC local
  _DST local
  date "+%Y%m%d_%H%M%S" _TS local
  $_SRC $_DST "/" $_TS ".tar.gz" suffix suffix suffix path-join tar -czf
  "Backup created: $_DST/$_TS.tar.gz" echo
] :backup

/home/user/data /backups backup
```

---

## 5. Development Workflows

### Build and Test

**Problem:** Run build, then tests if build succeeds.

```bash
[cargo build] [cargo test] &&
```

**Full CI-style workflow:**

```bash
[
  fmt cargo
  clippy cargo
  test cargo
  "All checks passed!" echo
] :ci

ci
```

---

### Watch Mode

**Problem:** Rebuild on file changes.

```bash
"src/**/*.rs" [cargo build] watch
```

**With custom debounce:**

```bash
"src/**/*.rs" [cargo build] 500 watch
# 500ms debounce
```

**Watch and test:**

```bash
"src/**/*.rs" [cargo test -- --nocapture] watch
```

---

### Git Operations

**Problem:** Stage and review changes interactively.

```bash
# See what's changed
--short status git spread
.s

# Stage specific files (inspect, then add)
--short status git spread
[" " split1 swap drop] each
[add git] each
```

**Quick commit workflow:**

```bash
[-a -m commit git] :qc
"Fix typo in README" qc
```

**View recent commits:**

```bash
--oneline -20 log git spread
```

---

### Dependency Management

**Problem:** Check for outdated dependencies.

```bash
# Rust/Cargo
cargo outdated

# Node.js
npm outdated

# Python (with pip-tools)
pip list --outdated
```

**Update and test:**

```bash
[cargo update] [cargo test] &&
```

---

## 6. API Scripting

### REST API Calls

**Problem:** Make authenticated API requests.

```bash
# GET request
"https://api.example.com/users" fetch

# POST with JSON body
'{"name": "Alice"}' "https://api.example.com/users" POST fetch

# With headers
marker "Authorization" "Bearer $TOKEN" record
'{"data": "test"}' "https://api.example.com/resource" POST fetch
```

---

### Pagination Handling

**Problem:** Fetch all pages of results.

```bash
[
  _BASE_URL local
  marker
  1 _PAGE local
  [true] [
    "$_BASE_URL?page=$_PAGE&limit=100" fetch
    dup "items" get dup count 0 gt?
    [spread $_PAGE 1 plus _PAGE local]
    [drop break]
    if
  ] while
  collect
] :fetch-paginated

"https://api.example.com/items" fetch-paginated
```

---

### Authentication

**Problem:** Handle OAuth token refresh.

```bash
[
  "https://auth.example.com/token" POST
  '{"grant_type": "refresh_token", "refresh_token": "..."}' swap
  fetch
  "access_token" get
] :refresh-token

refresh-token _TOKEN local
```

---

### Data Aggregation

**Problem:** Fetch from multiple APIs and combine.

```bash
[
  [https://api1.example.com/data fetch]
  [https://api2.example.com/data fetch]
  [https://api3.example.com/data fetch]
] parallel

# All three results now on stack
marker swap swap swap collect
```

---

## 7. One-Liners

### Quick Data Transformations

```bash
# Sum numbers from stdin
cat into-lines [[parse-num] map] sum

# Sort and dedupe
data.txt cat into-lines unique "name" sort-by

# JSON to CSV
data.json open to-csv

# CSV to JSON
data.csv open to-json
```

---

### File Statistics

```bash
# Count files by extension
. -type f find spread [basename "." rsplit1 swap drop] each unique count

# Total size of current directory
. du -sh

# Newest modified file
. ls-table "modified" sort-by reverse 1 first "name" get

# Count lines of code
*.rs find spread [wc -l] each into-lines sum
```

---

### Network Checks

```bash
# Check if host is up
"https://example.com" HEAD fetch-status 200 eq?

# Ping multiple hosts
["api.example.com" "db.example.com" "cache.example.com"] spread [ping -c 1] each

# Parallel health checks
[
  [api.example.com ping -c 1]
  [db.example.com ping -c 1]
  [cache.example.com ping -c 1]
] parallel
```

---

### String Manipulation

```bash
# Reverse a string
"hello" reverse
# Result: "olleh"

# Extract domain from URL
"https://www.example.com/path" "/" split1 swap drop "/" split1 swap drop "/" split1 drop

# Generate slug
"Hello World!" lower " " "-" str-replace

# Pad with zeros
"42" 5 "0" pad-left
# Result: "00042"

# Count words
"the quick brown fox" " " split count
```

---

### Math Operations

```bash
# Calculate average
'[1, 2, 3, 4, 5]' into-json avg
# Result: 3

# Find median
'[3, 1, 4, 1, 5, 9, 2, 6]' into-json "x" sort-by dup count 2 div nth

# Standard deviation
'[1, 2, 3, 4, 5]' into-json dup avg _MEAN local
[[_MEAN minus dup mul] map] sum count div sqrt

# Factorial
[
  dup 1 le?
  [drop 1]
  [dup 1 minus factorial mul]
  if
] :factorial

5 factorial
# Result: 120
```

---

### Vector/Embedding Operations

```bash
# Cosine similarity between two vectors
'[1, 2, 3]' into-json '[4, 5, 6]' into-json cosine-similarity

# Normalize a vector
'[3, 4]' into-json normalize
# Result: [0.6, 0.8]

# Dot product
'[1, 2, 3]' into-json '[4, 5, 6]' into-json dot-product
# Result: 32
```

---

## Quick Reference

| Task | One-liner |
|------|-----------|
| Count files | `*.txt ls spread depth` |
| Sum numbers | `file.txt cat into-lines sum` |
| Filter lines | `log.txt cat into-lines [["ERROR" contains?] filter]` |
| Transform each | `data spread [operation] each collect` |
| Parallel exec | `[[cmd1] [cmd2] [cmd3]] parallel` |
| JSON query | `file.json open "path.to.field" get` |
| Watch files | `"*.rs" [cargo build] watch` |
| Retry on fail | `3 [unreliable-command] retry` |
| HTTP GET | `"https://api.example.com" fetch` |
| Merge records | `rec1 rec2 merge` |

---

## Tips

1. **Use `.s` to inspect the stack** - Invaluable for debugging
2. **Start with `marker`** - Before operations that produce multiple values
3. **Blocks are cheap** - Define reusable operations with `:name`
4. **Try before you commit** - Preview operations before destructive actions
5. **Use `--trace`** - Debug complex pipelines with `hsab --trace`
6. **Combine with Unix tools** - hsab enhances, doesn't replace
