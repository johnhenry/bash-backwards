# Structured Data in hsab

hsab treats structured data as first-class values. Vectors (lists) and records (maps) live on the stack just like strings, enabling powerful data manipulation workflows.

---

## 1. Vectors (Lists)

Vectors are ordered collections of values.

### Creating Vectors

**From JSON strings:**

```bash
'[1, 2, 3]' json                    # Parse JSON array to List
'["a", "b", "c"]' json              # List of strings
'[1, "mixed", true]' json           # Mixed types
```

**Using `marker` and `collect`:**

```bash
marker a b c collect                # Creates a list from stack items
marker 1 2 3 [2 mul] each collect   # Transform and collect: [2, 4, 6]
```

**From command output:**

```bash
ls spread                           # Directory listing as stack items
"one\ntwo\nthree" into-lines        # Split string into list
```

### Indexing

**`nth` - Get element by index (0-based):**

```bash
'[10, 20, 30]' json 0 nth           # 10
'[10, 20, 30]' json 2 nth           # 30
'[10, 20, 30]' json 5 nth           # null (out of bounds)
```

**`first` - Get first N elements:**

```bash
'[1, 2, 3, 4, 5]' json 2 first      # [1, 2]
'[1, 2, 3]' json 1 first            # [1]
```

**`last` - Get last N elements:**

```bash
'[1, 2, 3, 4, 5]' json 2 last       # [4, 5]
'[1, 2, 3]' json 1 last             # [3]
```

### Length

**`count` - Get number of elements:**

```bash
'[1, 2, 3, 4, 5]' json count        # 5
'[]' json count                      # 0
```

**`len` - String length (works on stringified values):**

```bash
"hello" len                          # 5
```

### Slicing

**`slice` - Extract substring by position:**

```bash
"hello" 1 3 slice                   # "ell" (start at 1, take 3 chars)
"abcdef" 0 2 slice                  # "ab"
```

For list slicing, combine `first` and `last`:

```bash
# Get elements 2-4 from a list
'[0, 1, 2, 3, 4, 5]' json 5 first 3 last    # [2, 3, 4]
```

### Modifying Vectors

**Concatenation with `flatten`:**

```bash
'[[1, 2], [3, 4]]' json flatten     # [1, 2, 3, 4]
```

**Reversing:**

```bash
'[1, 2, 3]' json reverse            # [3, 2, 1]
```

**Deduplication:**

```bash
'[1, 2, 2, 3, 3, 3]' json unique    # [1, 2, 3]
```

**Finding duplicates:**

```bash
'[1, 2, 2, 3, 3, 3]' json duplicates # [2, 3]
```

---

## 2. Records (Maps)

Records are key-value collections, similar to JSON objects.

### Creating Records

**From JSON strings:**

```bash
'{"name": "Alice", "age": 30}' json
'{"nested": {"value": 42}}' json
```

**Using `record` builtin:**

```bash
"name" "Alice" "age" 30 record      # {name: Alice, age: 30}
"key" "value" record                 # {key: value}
```

**With marker for multiple fields:**

```bash
marker "a" 1 "b" 2 "c" 3 record     # {a: 1, b: 2, c: 3}
```

**From key=value text:**

```bash
"name=Alice\nage=30" into-kv        # {name: Alice, age: 30}
```

### Field Access

**`get` - Access a field:**

```bash
'{"name": "Alice"}' json "name" get          # Alice
'{"user": {"id": 42}}' json "user" get       # {id: 42}
```

**Nested access with dot notation:**

```bash
'{"server": {"host": "localhost", "port": 8080}}' json "server.port" get    # 8080
'{"items": [10, 20, 30]}' json "items.1" get                                 # 20
'{"users": [{"name": "Alice"}]}' json "users.0.name" get                    # Alice
```

**`has?` - Check if field exists:**

```bash
'{"name": "test"}' json "name" has?          # exit code 0 (true)
'{"name": "test"}' json "missing" has?       # exit code 1 (false)
```

### Modifying Records

**`set` - Add or update a field:**

```bash
'{"a": 1}' json "b" 2 set                    # {a: 1, b: 2}
'{"a": 1}' json "a" 99 set                   # {a: 99}
```

**Nested set with dot notation:**

```bash
'{"server": {"host": "localhost"}}' json "server.port" 9090 set
# {server: {host: localhost, port: 9090}}

'{}' json "a.b.c" "deep" set                 # {a: {b: {c: deep}}}
```

**`del` - Remove a field:**

```bash
'{"a": 1, "b": 2}' json "a" del              # {b: 2}
```

### Keys and Values

**`keys` - Get list of keys:**

```bash
'{"a": 1, "b": 2}' json keys                 # [a, b]
```

**`values` - Get list of values:**

```bash
'{"a": 1, "b": 2}' json values               # [1, 2]
```

### Merging Records

**`merge` - Combine two records (right overwrites left):**

```bash
'{"a": 1}' json '{"b": 2}' json merge        # {a: 1, b: 2}
'{"a": 1}' json '{"a": 99}' json merge       # {a: 99}
```

---

## 3. Spread and Extraction

### `spread` - Explode onto Stack

**For lists:**

```bash
'["a", "b", "c"]' json spread
# Stack: marker, "a", "b", "c"
```

**For records (values only, order undefined):**

```bash
'{"x": 1, "y": 2}' json spread
# Stack: marker, 1, 2 (order may vary)
```

**For strings (split by newlines):**

```bash
"line1\nline2\nline3" spread
# Stack: marker, "line1", "line2", "line3"
```

### `fields` - Named Extraction

Extract specific fields from a record without a marker:

```bash
'{"name": "Alice", "age": 30, "city": "NYC"}' json ["name" "age"] fields
# Stack: "Alice", 30
```

### `fields-keys` - Key-Value Pairs

Extract key-value pairs with a marker:

```bash
'{"a": 1, "b": 2}' json fields-keys
# Stack: marker, "a", 1, "b", 2
```

### `spread-head` - Split First Element

```bash
'[1, 2, 3, 4]' json spread-head
# Stack: 1, [2, 3, 4]
```

### `spread-tail` - Split Last Element

```bash
'[1, 2, 3, 4]' json spread-tail
# Stack: [1, 2, 3], 4
```

### `spread-n` - Take First N Elements

```bash
'[1, 2, 3, 4, 5]' json 2 spread-n
# Stack: 1, 2, [3, 4, 5]
```

### `spread-to` - Bind to Local Variables

Destructure values into named locals:

```bash
# From a list
'[10, 20, 30]' json ["_X" "_Y" "_Z"] spread-to
$_X echo    # 10
$_Y echo    # 20
$_Z echo    # 30

# From a record (extracts by key name)
'{"name": "Alice", "age": 30}' json ["name" "age"] spread-to
# Binds $name and $age
```

---

## 4. JSON Interop

### Parsing JSON

**`json` or `into-json` - Parse JSON string:**

```bash
'{"key": "value"}' json             # Parse to Record
'[1, 2, 3]' json                    # Parse to List
'42' json                           # Parse to Number
'"hello"' json                      # Parse to String
'true' json                         # Parse to Boolean
'null' json                         # Parse to Null
```

### Stringifying to JSON

**`unjson` or `to-json` - Convert to JSON string:**

```bash
"name" "test" record to-json        # {"name":"test"}
'[1, 2, 3]' json to-json            # [1,2,3]
```

### Working with API Responses

```bash
# Fetch and parse JSON API
[curl -s https://api.example.com/data] @ json

# Extract specific field
[curl -s https://api.example.com/user/123] @ json "name" get

# Deep extraction
[curl -s https://api.example.com/response] @ json "data.items.0.id" get

# Process list of results
[curl -s https://api.example.com/users] @ json spread [["name" "email"] fields] each
```

### File I/O with Auto-Format

**`open` - Read and parse by extension:**

```bash
"data.json" open                    # Auto-parses as JSON
"users.csv" open                    # Auto-parses as Table
"config.tsv" open                   # Auto-parses as Table
"readme.txt" open                   # Plain text
```

**`save` - Write with auto-format:**

```bash
'{"name": "test"}' json "output.json" save    # Writes formatted JSON
table "data.csv" save                          # Writes CSV
```

---

## 5. Common Patterns

### Transforming Lists of Records

**Map over records:**

```bash
# Double each age
'[{"name":"A","age":20},{"name":"B","age":30}]' json
spread [dup "age" get 2 mul "age" swap set] each collect
```

**Extract a single field from all records:**

```bash
'[{"name":"Alice"},{"name":"Bob"}]' json
spread ["name" get] each collect
# ["Alice", "Bob"]
```

### Filtering by Field

**Using `keep` with spread items:**

```bash
# Keep users over 25
'[{"name":"A","age":20},{"name":"B","age":30}]' json
spread ["age" get 25 gt?] keep collect
```

**Using `where` on tables:**

```bash
# Filter table rows
marker
    "name" "alice" "age" 30 record
    "name" "bob" "age" 25 record
    "name" "carol" "age" 35 record
table
["age" get 30 gt?] where
# Table with only carol
```

**Using `reject` (inverse of keep):**

```bash
# Remove users under 25
'[{"name":"A","age":20},{"name":"B","age":30}]' json
spread ["age" get 25 lt?] reject collect
```

### Building Records from Stack Values

**Manual construction:**

```bash
# name age email -> record
"email" swap "age" swap "name" swap record
```

**Using a definition:**

```bash
# Define a constructor
["_email" local "_age" local "_name" local
 "name" $_name "age" $_age "email" $_email record] :make-user

"Alice" 30 "alice@example.com" make-user
# {name: Alice, age: 30, email: alice@example.com}
```

### Aggregating Data

**Sum, average, min, max:**

```bash
'[1, 2, 3, 4, 5]' json sum          # 15
'[10, 20, 30]' json avg             # 20
'[5, 2, 8, 1, 9]' json min          # 1
'[5, 2, 8, 1, 9]' json max          # 9
```

**Custom aggregation with `reduce`:**

```bash
# Product
'[2, 3, 4]' json 1 [mul] reduce     # 24

# Concatenate strings
'["a", "b", "c"]' json "" [suffix] reduce    # "abc"
```

**Group by field:**

```bash
marker
    "type" "fruit" "name" "apple" record
    "type" "veg" "name" "carrot" record
    "type" "fruit" "name" "banana" record
table
"type" group-by
# {fruit: <table>, veg: <table>}

# Access a group
"fruit" get count                   # 2
```

### Sorting

**Sort table by column:**

```bash
marker
    "name" "Bob" record
    "name" "Alice" record
table
"name" sort-by
# Table with Alice first
```

**Sort list of records by field:**

```bash
'[{"age":30},{"age":20},{"age":25}]' json "age" sort-by
# Sorted by age ascending
```

### Selecting Columns

```bash
marker
    "name" "alice" "age" 30 "city" "NYC" record
table
["name" "age"] select
# Table with only name and age columns
```

---

## Quick Reference

| Operation | Stack Effect | Description |
|-----------|--------------|-------------|
| `json` | string -> value | Parse JSON |
| `to-json` | value -> string | Stringify to JSON |
| `record` | k1 v1 k2 v2 ... -> record | Create record |
| `get` | record key -> value | Get field |
| `set` | record key value -> record | Set field |
| `del` | record key -> record | Delete field |
| `has?` | record key -> (exit code) | Check field exists |
| `keys` | record -> list | Get all keys |
| `values` | record -> list | Get all values |
| `merge` | rec1 rec2 -> record | Combine records |
| `spread` | value -> marker items... | Explode onto stack |
| `collect` | marker items... -> list | Gather into list |
| `fields` | record [keys] -> values... | Extract named fields |
| `fields-keys` | record -> marker k v k v... | Extract pairs |
| `spread-head` | list -> head tail-list | Split first |
| `spread-tail` | list -> init-list last | Split last |
| `spread-n` | list n -> items... rest | Take first n |
| `spread-to` | value [names] -> (binds) | Destructure to locals |
| `first` | list n -> list | First n elements |
| `last` | list n -> list | Last n elements |
| `nth` | list n -> value | Element at index |
| `count` | list -> number | Number of elements |
| `reverse` | list -> list | Reverse order |
| `unique` | list -> list | Remove duplicates |
| `flatten` | nested-list -> list | Flatten one level |
| `sum` | list -> number | Sum numbers |
| `avg` | list -> number | Average |
| `min` | list -> number | Minimum |
| `max` | list -> number | Maximum |
| `reduce` | list init [block] -> value | Custom aggregation |
| `sort-by` | list/table key -> sorted | Sort by field |
| `group-by` | table col -> record | Group rows |
| `where` | table [pred] -> table | Filter rows |
| `select` | table [cols] -> table | Select columns |
| `keep` | marker items... [pred] -> filtered | Filter items |
| `reject` | list [pred] -> list | Remove matching |
| `each` | marker items... [block] -> results... | Transform each |
| `map` | marker items... [block] -> list | Transform + collect |
| `filter` | marker items... [pred] -> list | Filter + collect |
