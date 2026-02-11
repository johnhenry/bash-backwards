# hsab Object Model - Detailed Implementation Plan

## Current State

hsab already has foundational types in `src/ast.rs`:

```rust
pub enum Value {
    Literal(String),    // String
    Output(String),     // Command output
    Block(Vec<Expr>),   // Deferred execution
    Nil,                // Absence
    Marker,             // Stack boundary
    List(Vec<Value>),   // ✓ Already exists
    Map(HashMap<String, Value>),  // ✓ Already exists (= Record)
    Number(f64),        // ✓ Already exists
    Bool(bool),         // ✓ Already exists
}
```

**What's missing:**
- Table type (list of records with consistent columns)
- Error type (structured errors)
- `into`/`to-*` serialization bridge
- Structured built-ins (ls, ps returning Tables)
- Type-aware display formatting

---

## Phase 0: Value Representation Cleanup

### 0.1 Add Table and Error Types

**File: `src/ast.rs`**

```rust
pub enum Value {
    // ... existing types ...

    /// A table: list of records with consistent columns
    /// Stored as (columns, rows) where rows are Vec<Value> in column order
    Table {
        columns: Vec<String>,
        rows: Vec<Vec<Value>>,
    },

    /// Structured error
    Error {
        kind: String,
        message: String,
        code: Option<i32>,
        source: Option<String>,
        command: Option<String>,
    },
}
```

### 0.2 Add `typeof` Builtin

**File: `src/eval.rs`**

```rust
fn builtin_typeof(&mut self) -> Result<(), EvalError> {
    let val = self.stack.pop().ok_or_else(||
        EvalError::StackUnderflow("typeof".into()))?;

    let type_name = match val {
        Value::Literal(_) | Value::Output(_) => "String",
        Value::Number(_) => "Number",
        Value::Bool(_) => "Boolean",
        Value::List(_) => "List",
        Value::Map(_) => "Record",
        Value::Table { .. } => "Table",
        Value::Block(_) => "Block",
        Value::Nil => "Null",
        Value::Marker => "Marker",
        Value::Error { .. } => "Error",
    };

    self.stack.push(Value::Literal(type_name.to_string()));
    Ok(())
}
```

### 0.3 Tests

```rust
#[test]
fn test_typeof_string() {
    let mut eval = Evaluator::new();
    eval.eval_line("\"hello\" typeof").unwrap();
    assert_eq!(eval.stack.pop().unwrap().as_arg(), Some("String".into()));
}

#[test]
fn test_typeof_number() {
    let mut eval = Evaluator::new();
    eval.eval_line("42 typeof").unwrap();
    assert_eq!(eval.stack.pop().unwrap().as_arg(), Some("Number".into()));
}

#[test]
fn test_typeof_record() {
    let mut eval = Evaluator::new();
    eval.eval_line("\"name\" \"hsab\" record typeof").unwrap();
    assert_eq!(eval.stack.pop().unwrap().as_arg(), Some("Record".into()));
}
```

---

## Phase 1: Record Operations

### 1.1 Record Construction

**Builtins to add:**

| Builtin | Stack Effect | Description |
|---------|--------------|-------------|
| `record` | `k1 v1 k2 v2 ... → Record` | Collect pairs into record |
| `into-kv` | `"k=v\nk2=v2" → Record` | Parse key=value text |

**Implementation:**

```rust
fn builtin_record(&mut self) -> Result<(), EvalError> {
    // Collect pairs from stack until marker or empty
    let mut pairs = Vec::new();

    while self.stack.len() >= 2 {
        if matches!(self.stack.last(), Some(Value::Marker)) {
            self.stack.pop(); // consume marker
            break;
        }
        let value = self.stack.pop().unwrap();
        let key = self.stack.pop()
            .and_then(|v| v.as_arg())
            .ok_or_else(|| EvalError::TypeError {
                expected: "String key".into(),
                got: "non-string".into(),
            })?;
        pairs.push((key, value));
    }

    pairs.reverse(); // Restore original order
    let map: HashMap<String, Value> = pairs.into_iter().collect();
    self.stack.push(Value::Map(map));
    Ok(())
}
```

### 1.2 Field Access

| Builtin | Stack Effect | Description |
|---------|--------------|-------------|
| `get` | `Record "key" → value` | Get field value |
| `set` | `Record "key" value → Record` | Set field (immutable) |
| `del` | `Record "key" → Record` | Remove field |
| `has?` | `Record "key" → Bool` | Check if field exists |
| `keys` | `Record → List` | Get all keys |
| `values` | `Record → List` | Get all values |
| `merge` | `Record Record → Record` | Merge (right overwrites) |

### 1.3 Tests

```rust
#[test]
fn test_record_construction() {
    let mut eval = Evaluator::new();
    eval.eval_line("\"name\" \"hsab\" \"version\" \"0.2\" record").unwrap();
    let val = eval.stack.pop().unwrap();
    assert!(matches!(val, Value::Map(_)));
}

#[test]
fn test_record_get() {
    let mut eval = Evaluator::new();
    eval.eval_line("\"name\" \"hsab\" record \"name\" get").unwrap();
    assert_eq!(eval.stack.pop().unwrap().as_arg(), Some("hsab".into()));
}

#[test]
fn test_record_set() {
    let mut eval = Evaluator::new();
    eval.eval_line("\"a\" 1 record \"b\" 2 set \"b\" get").unwrap();
    // Should have value 2
}

#[test]
fn test_record_merge() {
    let mut eval = Evaluator::new();
    eval.eval_line("\"a\" 1 record \"b\" 2 record merge keys").unwrap();
    // Should have both a and b
}
```

---

## Phase 2: Tables and Lists

### 2.1 Table Type

A Table is stored as:
```rust
Value::Table {
    columns: Vec<String>,        // ["name", "age", "city"]
    rows: Vec<Vec<Value>>,       // [[alice, 30, NYC], [bob, 25, LA]]
}
```

### 2.2 Table Construction

| Builtin | Stack Effect | Description |
|---------|--------------|-------------|
| `table` | `Record Record ... → Table` | Records to table |
| `into-csv` | `"csv text" → Table` | Parse CSV |
| `into-tsv` | `"tsv text" → Table` | Parse TSV |
| `into-json` | `"[{...}]" → Table` | Parse JSON array |

### 2.3 Column Operations

| Builtin | Stack Effect | Description |
|---------|--------------|-------------|
| `select` | `Table [cols] → Table` | Keep only listed columns |
| `drop-cols` | `Table [cols] → Table` | Remove columns |
| `rename-col` | `Table "old" "new" → Table` | Rename column |
| `add-col` | `Table "name" [block] → Table` | Add computed column |

### 2.4 Row Operations

| Builtin | Stack Effect | Description |
|---------|--------------|-------------|
| `where` | `Table [pred] → Table` | Filter rows |
| `sort-by` | `Table "col" → Table` | Sort by column |
| `first` | `Table n → Table` | First n rows |
| `last` | `Table n → Table` | Last n rows |
| `nth` | `Table n → Record` | Get row as record |
| `group-by` | `Table "col" → Record` | Group into {val: Table} |
| `unique` | `Table → Table` | Deduplicate rows |

### 2.5 List Operations

| Builtin | Stack Effect | Description |
|---------|--------------|-------------|
| `list` | `a b c ... → List` | Collect to list |
| `into-lines` | `"a\nb\nc" → List` | Split by newlines |
| `into-words` | `"a b c" → List` | Split by whitespace |
| `nth` | `List n → value` | Get item at index |
| `length` | `List → Number` | Get length |
| `map` | `List [block] → List` | Transform each |
| `filter` | `List [pred] → List` | Keep matching |
| `reverse` | `List → List` | Reverse order |
| `sort` | `List → List` | Sort items |
| `flatten` | `List → List` | Flatten nested |
| `unique` | `List → List` | Deduplicate |

---

## Phase 3: Structured Errors

### 3.1 Error Type

```rust
Value::Error {
    kind: String,      // "command_failed", "parse_error", "type_error"
    message: String,   // Human-readable message
    code: Option<i32>, // Exit code if command failure
    source: Option<String>,  // Input that caused error
    command: Option<String>, // Command that failed
}
```

### 3.2 Error Handling Builtins

| Builtin | Stack Effect | Description |
|---------|--------------|-------------|
| `try` | `[block] → result or Error` | Run block, catch errors |
| `error?` | `value → Bool` | Is this an Error? |
| `throw` | `"message" → Error` | Create error |

### 3.3 Error Propagation

When an Error is on top of stack and not handled:
1. If interactive, print error and continue
2. If in script, propagate up (like `set -e`)
3. `try` captures errors instead of propagating

---

## Phase 4: External Command Interop

### 4.1 Auto-Serialization (Structured → External)

When piping to external command, auto-serialize:

| Type | Serialization |
|------|---------------|
| String | as-is |
| Number | decimal string |
| Bool | "true" / "false" |
| Record | `key=value` lines |
| Table | TSV |
| List | newline-separated |
| Null | empty string |
| Error | message string |

**Implementation in `src/eval.rs`:**

```rust
fn serialize_for_external(&self, value: &Value) -> String {
    match value {
        Value::Literal(s) | Value::Output(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        Value::Map(m) => {
            m.iter()
                .map(|(k, v)| format!("{}={}", k, self.serialize_for_external(v)))
                .collect::<Vec<_>>()
                .join("\n")
        }
        Value::Table { columns, rows } => {
            let mut lines = vec![columns.join("\t")];
            for row in rows {
                let line = row.iter()
                    .map(|v| self.serialize_for_external(v))
                    .collect::<Vec<_>>()
                    .join("\t");
                lines.push(line);
            }
            lines.join("\n")
        }
        Value::List(items) => {
            items.iter()
                .map(|v| self.serialize_for_external(v))
                .collect::<Vec<_>>()
                .join("\n")
        }
        Value::Nil => String::new(),
        Value::Error { message, .. } => message.clone(),
        _ => String::new(),
    }
}
```

### 4.2 Explicit Parsing (External → Structured)

| Builtin | Stack Effect | Description |
|---------|--------------|-------------|
| `into-json` | `String → Record/Table/List` | Parse JSON |
| `into-csv` | `String → Table` | Parse CSV |
| `into-tsv` | `String → Table` | Parse TSV |
| `into-lines` | `String → List` | Split on newlines |
| `into-words` | `String → List` | Split on whitespace |
| `into-kv` | `String → Record` | Parse key=value pairs |
| `into-cols` | `String → Table` | Auto-detect columns |

### 4.3 Explicit Serialization

| Builtin | Stack Effect | Description |
|---------|--------------|-------------|
| `to-json` | `Record/Table → String` | Serialize to JSON |
| `to-csv` | `Table → String` | Serialize to CSV |
| `to-tsv` | `Table → String` | Serialize to TSV |
| `to-lines` | `List → String` | Join with newlines |
| `to-kv` | `Record → String` | Serialize to key=value |
| `to-md` | `Table → String` | Serialize to Markdown table |

---

## Phase 5: Structured Built-ins

### 5.1 File System

| Builtin | Returns | Columns |
|---------|---------|---------|
| `ls` | Table | name, type, size, modified, permissions |
| `glob` | Table | name, path, size, modified |
| `du` | Table | path, size |
| `df` | Table | filesystem, size, used, available, mount |

### 5.2 Process Management

| Builtin | Returns | Columns |
|---------|---------|---------|
| `ps` | Table | pid, name, cpu, mem, user, started |

### 5.3 Network/Files

| Builtin | Returns | Description |
|---------|---------|-------------|
| `fetch` | String or Table | HTTP GET (--json for auto-parse) |
| `open` | Record/Table | Auto-parse by extension |

### 5.4 System

| Builtin | Returns | Description |
|---------|---------|-------------|
| `env` | Record | All environment variables |
| `which` | Record | path, version, type |

---

## Phase 6: Advanced Operations (Deferred)

- `inner-join`, `left-join`, `cross-join`
- `pivot`, `unpivot`
- `rolling`, `rank`
- Providers (fs:, env:, docker:, k8s:, git:)

---

## Phase 7: REPL Enhancements (Deferred)

- Stack preview with type summaries
- Tab completion for field names
- `--trace` mode
- `peek`/`inspect` for detailed view

---

## Implementation Order

```
Week 1-2: Phase 0 (Value types, typeof)
Week 3-4: Phase 1 (Records: record, get, set, merge)
Week 5-6: Phase 2 (Tables: table, where, sort-by, select)
Week 7-8: Phase 3 (Errors: try, error?, throw)
Week 9-10: Phase 4 (Interop: into-*, to-*)
Week 11-12: Phase 5 (Structured ls, ps, open)
```

Each phase ships independently. Phase 4 (interop) is critical for usability.

---

## File Changes Summary

| File | Changes |
|------|---------|
| `src/ast.rs` | Add Table, Error variants to Value enum |
| `src/eval.rs` | All new builtins, serialization logic |
| `src/display.rs` | **NEW** - Format-on-display for Record/Table |
| `src/parser.rs` | into-json, into-csv parsing |
| `src/main.rs` | REPL integration, type-aware stack hints |
| `tests/` | Comprehensive tests for each phase |
