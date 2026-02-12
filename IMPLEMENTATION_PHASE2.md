# hsab Phase 2 - Extended Features Implementation Plan

## Overview

Building on the structured data foundation (Phases 0-4), this plan adds stack utilities, aggregations, display formatting, and convenience features.

---

## Phase 5: Stack Utilities

### 5.1 `peek` - View stack without popping

**Stack Effect:** `... → ...` (no change, prints to stderr)

**Behavior:**
- `peek` - show top item
- `N peek` - show top N items
- Output goes to stderr so it doesn't interfere with pipelines

**Implementation:**

```rust
fn builtin_peek(&mut self) -> Result<(), EvalError> {
    // Check if there's a number on top specifying count
    let count = match self.stack.last() {
        Some(Value::Number(n)) => {
            let c = *n as usize;
            self.stack.pop();
            c
        }
        Some(Value::Literal(s)) if s.parse::<usize>().is_ok() => {
            let c = s.parse().unwrap();
            self.stack.pop();
            c
        }
        _ => 1,
    };

    let len = self.stack.len();
    let start = len.saturating_sub(count);

    eprintln!("--- peek ({} items) ---", count.min(len));
    for (i, val) in self.stack.iter().skip(start).enumerate() {
        eprintln!("[{}]: {}", start + i, format_value_short(val));
    }
    eprintln!("---");

    Ok(())
}
```

### 5.2 `tap` - Side effect without consuming

**Stack Effect:** `a [block] → a`

**Behavior:**
- Pop block, peek at top value (don't pop)
- Execute block with value on stack
- Discard block's results, restore original top value

```rust
fn builtin_tap(&mut self) -> Result<(), EvalError> {
    let block = self.pop_block()?;
    let value = self.stack.last()
        .ok_or_else(|| EvalError::StackUnderflow("tap".into()))?
        .clone();

    // Execute block (value is still on stack)
    for expr in &block {
        self.eval_expr(expr)?;
    }

    // Clear any results the block produced, restore original
    while self.stack.last() != Some(&value) && !self.stack.is_empty() {
        self.stack.pop();
    }
    // If we accidentally popped the original, push it back
    if self.stack.last() != Some(&value) {
        self.stack.push(value);
    }

    Ok(())
}
```

**Simpler implementation** (just dup + execute + drop intermediate):

```rust
fn builtin_tap(&mut self) -> Result<(), EvalError> {
    let block = self.pop_block()?;

    // Remember stack depth
    let depth_before = self.stack.len();

    // Duplicate top for the block to consume
    self.builtin_dup()?;

    // Execute block
    for expr in &block {
        self.eval_expr(expr)?;
    }

    // Drop everything the block added (keep original)
    while self.stack.len() > depth_before {
        self.stack.pop();
    }

    Ok(())
}
```

### 5.3 `dip` - Execute block "under" top value

**Stack Effect:** `a b [block] → a (block results) b`

**Behavior:**
- Pop block and top value
- Execute block (operates on rest of stack)
- Push saved value back

```rust
fn builtin_dip(&mut self) -> Result<(), EvalError> {
    let block = self.pop_block()?;
    let saved = self.stack.pop()
        .ok_or_else(|| EvalError::StackUnderflow("dip".into()))?;

    // Execute block on remaining stack
    for expr in &block {
        self.eval_expr(expr)?;
    }

    // Restore saved value
    self.stack.push(saved);
    Ok(())
}
```

---

## Phase 6: Aggregation Operations

### 6.1 `sum` - Sum numbers

**Stack Effect:** `List → Number` or `Table "col" → Number`

```rust
fn builtin_sum(&mut self) -> Result<(), EvalError> {
    let val = self.stack.pop()
        .ok_or_else(|| EvalError::StackUnderflow("sum".into()))?;

    let total = match val {
        Value::List(items) => {
            items.iter().filter_map(|v| match v {
                Value::Number(n) => Some(*n),
                Value::Literal(s) | Value::Output(s) => s.trim().parse().ok(),
                _ => None,
            }).sum()
        }
        _ => return Err(EvalError::TypeError {
            expected: "List".into(),
            got: format!("{:?}", val),
        }),
    };

    self.stack.push(Value::Number(total));
    Ok(())
}
```

### 6.2 `avg` - Average numbers

```rust
fn builtin_avg(&mut self) -> Result<(), EvalError> {
    let val = self.stack.pop()
        .ok_or_else(|| EvalError::StackUnderflow("avg".into()))?;

    let (total, count) = match val {
        Value::List(items) => {
            let nums: Vec<f64> = items.iter().filter_map(|v| match v {
                Value::Number(n) => Some(*n),
                Value::Literal(s) | Value::Output(s) => s.trim().parse().ok(),
                _ => None,
            }).collect();
            (nums.iter().sum::<f64>(), nums.len())
        }
        _ => return Err(EvalError::TypeError {
            expected: "List".into(),
            got: format!("{:?}", val),
        }),
    };

    let avg = if count > 0 { total / count as f64 } else { 0.0 };
    self.stack.push(Value::Number(avg));
    Ok(())
}
```

### 6.3 `min` / `max` - Extrema

```rust
fn builtin_min(&mut self) -> Result<(), EvalError> {
    let val = self.stack.pop()
        .ok_or_else(|| EvalError::StackUnderflow("min".into()))?;

    let result = match val {
        Value::List(items) => {
            items.iter().filter_map(|v| match v {
                Value::Number(n) => Some(*n),
                Value::Literal(s) | Value::Output(s) => s.trim().parse().ok(),
                _ => None,
            }).fold(f64::INFINITY, f64::min)
        }
        _ => return Err(EvalError::TypeError {
            expected: "List".into(),
            got: format!("{:?}", val),
        }),
    };

    if result.is_infinite() {
        self.stack.push(Value::Nil);
    } else {
        self.stack.push(Value::Number(result));
    }
    Ok(())
}

fn builtin_max(&mut self) -> Result<(), EvalError> {
    // Same but with f64::NEG_INFINITY and f64::max
}
```

### 6.4 `count` - Count items

```rust
fn builtin_count(&mut self) -> Result<(), EvalError> {
    let val = self.stack.pop()
        .ok_or_else(|| EvalError::StackUnderflow("count".into()))?;

    let n = match &val {
        Value::List(items) => items.len(),
        Value::Table { rows, .. } => rows.len(),
        Value::Literal(s) | Value::Output(s) => s.lines().count(),
        _ => 1,
    };

    self.stack.push(Value::Number(n as f64));
    Ok(())
}
```

---

## Phase 7: Deep Path Access

### 7.1 Enhanced `get` with dot notation

**Stack Effect:** `Record "path.to.field" → value`

Modify `builtin_get` to support dot-separated paths:

```rust
fn builtin_get(&mut self) -> Result<(), EvalError> {
    let key = self.pop_string()?;
    let val = self.stack.pop()
        .ok_or_else(|| EvalError::StackUnderflow("get".into()))?;

    // Check if key contains dots
    if key.contains('.') {
        let result = self.deep_get(&val, &key)?;
        self.stack.push(result);
    } else {
        // Original single-level get
        let result = match val {
            Value::Map(map) => map.get(&key).cloned().unwrap_or(Value::Nil),
            Value::Table { columns, rows } => {
                // Get column as list
                if let Some(idx) = columns.iter().position(|c| c == &key) {
                    let values: Vec<Value> = rows.iter()
                        .map(|row| row.get(idx).cloned().unwrap_or(Value::Nil))
                        .collect();
                    Value::List(values)
                } else {
                    Value::Nil
                }
            }
            Value::List(items) => {
                // Numeric index
                if let Ok(idx) = key.parse::<usize>() {
                    items.get(idx).cloned().unwrap_or(Value::Nil)
                } else {
                    Value::Nil
                }
            }
            _ => Value::Nil,
        };
        self.stack.push(result);
    }
    Ok(())
}

fn deep_get(&self, val: &Value, path: &str) -> Result<Value, EvalError> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = val.clone();

    for part in parts {
        current = match current {
            Value::Map(map) => map.get(part).cloned().unwrap_or(Value::Nil),
            Value::List(items) => {
                if let Ok(idx) = part.parse::<usize>() {
                    items.get(idx).cloned().unwrap_or(Value::Nil)
                } else {
                    Value::Nil
                }
            }
            _ => Value::Nil,
        };
    }

    Ok(current)
}
```

---

## Phase 8: Table Operations Extended

### 8.1 `group-by` - Group rows by column

**Stack Effect:** `Table "col" → Record` (keys are column values, values are sub-tables)

```rust
fn builtin_group_by(&mut self) -> Result<(), EvalError> {
    let col = self.pop_string()?;
    let table = self.stack.pop()
        .ok_or_else(|| EvalError::StackUnderflow("group-by".into()))?;

    match table {
        Value::Table { columns, rows } => {
            let col_idx = columns.iter().position(|c| c == &col)
                .ok_or_else(|| EvalError::ExecError(
                    format!("group-by: column '{}' not found", col)
                ))?;

            let mut groups: HashMap<String, Vec<Vec<Value>>> = HashMap::new();

            for row in rows {
                let key = row.get(col_idx)
                    .and_then(|v| v.as_arg())
                    .unwrap_or_default();
                groups.entry(key).or_default().push(row);
            }

            // Convert groups to Record of Tables
            let map: HashMap<String, Value> = groups.into_iter()
                .map(|(k, rows)| {
                    (k, Value::Table { columns: columns.clone(), rows })
                })
                .collect();

            self.stack.push(Value::Map(map));
        }
        _ => return Err(EvalError::TypeError {
            expected: "Table".into(),
            got: format!("{:?}", table),
        }),
    }

    Ok(())
}
```

### 8.2 `add-column` - Add computed column

**Stack Effect:** `Table "name" [block] → Table`

```rust
fn builtin_add_column(&mut self) -> Result<(), EvalError> {
    let block = self.pop_block()?;
    let col_name = self.pop_string()?;
    let table = self.stack.pop()
        .ok_or_else(|| EvalError::StackUnderflow("add-column".into()))?;

    match table {
        Value::Table { mut columns, mut rows } => {
            columns.push(col_name);

            for row in &mut rows {
                // Create record from row
                let record: HashMap<String, Value> = columns.iter()
                    .zip(row.iter())
                    .map(|(c, v)| (c.clone(), v.clone()))
                    .collect();

                // Push record, execute block
                self.stack.push(Value::Map(record));
                for expr in &block {
                    self.eval_expr(expr)?;
                }

                // Pop result as new column value
                let new_val = self.stack.pop().unwrap_or(Value::Nil);
                row.push(new_val);
            }

            self.stack.push(Value::Table { columns, rows });
        }
        _ => return Err(EvalError::TypeError {
            expected: "Table".into(),
            got: format!("{:?}", table),
        }),
    }

    Ok(())
}
```

### 8.3 `unique` - Deduplicate

**Stack Effect:** `List → List` or `Table → Table`

```rust
fn builtin_unique(&mut self) -> Result<(), EvalError> {
    let val = self.stack.pop()
        .ok_or_else(|| EvalError::StackUnderflow("unique".into()))?;

    match val {
        Value::List(items) => {
            let mut seen = HashSet::new();
            let unique: Vec<Value> = items.into_iter()
                .filter(|v| {
                    let key = v.as_arg().unwrap_or_default();
                    seen.insert(key)
                })
                .collect();
            self.stack.push(Value::List(unique));
        }
        Value::Table { columns, rows } => {
            let mut seen = HashSet::new();
            let unique: Vec<Vec<Value>> = rows.into_iter()
                .filter(|row| {
                    let key: String = row.iter()
                        .filter_map(|v| v.as_arg())
                        .collect::<Vec<_>>()
                        .join("\t");
                    seen.insert(key)
                })
                .collect();
            self.stack.push(Value::Table { columns, rows: unique });
        }
        _ => self.stack.push(val),
    }

    Ok(())
}
```

### 8.4 `reverse` - Reverse order

**Stack Effect:** `List → List` or `Table → Table`

```rust
fn builtin_reverse(&mut self) -> Result<(), EvalError> {
    let val = self.stack.pop()
        .ok_or_else(|| EvalError::StackUnderflow("reverse".into()))?;

    match val {
        Value::List(mut items) => {
            items.reverse();
            self.stack.push(Value::List(items));
        }
        Value::Table { columns, mut rows } => {
            rows.reverse();
            self.stack.push(Value::Table { columns, rows });
        }
        Value::Literal(s) => {
            self.stack.push(Value::Literal(s.chars().rev().collect()));
        }
        _ => self.stack.push(val),
    }

    Ok(())
}
```

---

## Phase 9: Format-on-Display

### 9.1 Create `src/display.rs`

```rust
//! Display formatting for structured data types

use crate::ast::Value;

/// Format a value for terminal display
pub fn format_value(val: &Value, max_width: usize) -> String {
    match val {
        Value::Table { columns, rows } => format_table(columns, rows, max_width),
        Value::Map(map) => format_record(map, max_width),
        Value::List(items) => format_list(items, max_width),
        Value::Error { kind, message, code, .. } => {
            let code_str = code.map(|c| format!(" (exit {})", c)).unwrap_or_default();
            format!("Error[{}]: {}{}", kind, message, code_str)
        }
        _ => val.as_arg().unwrap_or_default(),
    }
}

fn format_table(columns: &[String], rows: &[Vec<Value>], max_width: usize) -> String {
    if columns.is_empty() || rows.is_empty() {
        return "(empty table)".to_string();
    }

    // Calculate column widths
    let mut widths: Vec<usize> = columns.iter().map(|c| c.len()).collect();
    for row in rows {
        for (i, val) in row.iter().enumerate() {
            if let Some(w) = widths.get_mut(i) {
                *w = (*w).max(val.as_arg().unwrap_or_default().len());
            }
        }
    }

    // Cap widths
    let total: usize = widths.iter().sum();
    if total > max_width {
        let scale = max_width as f64 / total as f64;
        for w in &mut widths {
            *w = ((*w as f64 * scale) as usize).max(3);
        }
    }

    let mut out = String::new();

    // Header
    out.push_str("┌");
    for (i, w) in widths.iter().enumerate() {
        out.push_str(&"─".repeat(*w + 2));
        out.push_str(if i < widths.len() - 1 { "┬" } else { "┐\n" });
    }

    out.push_str("│");
    for (i, col) in columns.iter().enumerate() {
        out.push_str(&format!(" {:width$} │", col, width = widths[i]));
    }
    out.push('\n');

    out.push_str("├");
    for (i, w) in widths.iter().enumerate() {
        out.push_str(&"─".repeat(*w + 2));
        out.push_str(if i < widths.len() - 1 { "┼" } else { "┤\n" });
    }

    // Rows
    for row in rows {
        out.push_str("│");
        for (i, val) in row.iter().enumerate() {
            let s = val.as_arg().unwrap_or_default();
            let w = widths.get(i).copied().unwrap_or(10);
            let truncated = if s.len() > w { format!("{}…", &s[..w-1]) } else { s };
            out.push_str(&format!(" {:width$} │", truncated, width = w));
        }
        out.push('\n');
    }

    // Footer
    out.push_str("└");
    for (i, w) in widths.iter().enumerate() {
        out.push_str(&"─".repeat(*w + 2));
        out.push_str(if i < widths.len() - 1 { "┴" } else { "┘" });
    }

    out
}

fn format_record(map: &std::collections::HashMap<String, Value>, _max_width: usize) -> String {
    let mut out = String::from("{\n");
    for (k, v) in map {
        out.push_str(&format!("  {}: {}\n", k, format_value_short(v)));
    }
    out.push('}');
    out
}

fn format_list(items: &[Value], _max_width: usize) -> String {
    if items.len() <= 5 {
        let parts: Vec<String> = items.iter()
            .map(|v| format_value_short(v))
            .collect();
        format!("[{}]", parts.join(", "))
    } else {
        let first: Vec<String> = items.iter().take(3)
            .map(|v| format_value_short(v))
            .collect();
        format!("[{}, ... ({} more)]", first.join(", "), items.len() - 3)
    }
}

pub fn format_value_short(val: &Value) -> String {
    match val {
        Value::Literal(s) => format!("\"{}\"", s),
        Value::Output(s) => s.trim().to_string(),
        Value::Number(n) => {
            if n.fract() == 0.0 { format!("{}", *n as i64) }
            else { format!("{:.2}", n) }
        }
        Value::Bool(b) => b.to_string(),
        Value::Nil => "nil".to_string(),
        Value::List(items) => format!("[...{}]", items.len()),
        Value::Map(_) => "{...}".to_string(),
        Value::Table { rows, .. } => format!("<table:{} rows>", rows.len()),
        Value::Block(_) => "[...]".to_string(),
        Value::Marker => "|marker|".to_string(),
        Value::Error { message, .. } => format!("Error: {}", message),
    }
}
```

### 9.2 Integration

In `main.rs`, use `format_value()` for REPL output display.

---

## Phase 10: HTTP Fetch

### 10.1 Add dependency

```toml
[dependencies]
ureq = "2"
```

### 10.2 `fetch` builtin

**Stack Effect:** `"url" → String`

```rust
fn builtin_fetch(&mut self) -> Result<(), EvalError> {
    let url = self.pop_string()?;

    let response = ureq::get(&url)
        .call()
        .map_err(|e| EvalError::ExecError(format!("fetch: {}", e)))?;

    let body = response.into_string()
        .map_err(|e| EvalError::ExecError(format!("fetch: {}", e)))?;

    self.stack.push(Value::Output(body));
    Ok(())
}
```

---

## Phase 11: Additional Parsers

### 11.1 `into-tsv` with custom delimiter

**Stack Effect:** `"text" "delim" → Table` or `"text" → Table` (default tab)

```rust
fn builtin_into_tsv(&mut self) -> Result<(), EvalError> {
    // Check if delimiter is provided
    let (text, delim) = if self.stack.len() >= 2 {
        if let Some(Value::Literal(d)) = self.stack.last() {
            if d.len() == 1 {
                let d = self.pop_string()?;
                let t = self.pop_string()?;
                (t, d)
            } else {
                (self.pop_string()?, "\t".to_string())
            }
        } else {
            (self.pop_string()?, "\t".to_string())
        }
    } else {
        (self.pop_string()?, "\t".to_string())
    };

    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        self.stack.push(Value::Table { columns: vec![], rows: vec![] });
        return Ok(());
    }

    let columns: Vec<String> = lines[0].split(&delim)
        .map(|s| s.trim().to_string())
        .collect();

    let rows: Vec<Vec<Value>> = lines[1..].iter()
        .map(|line| {
            line.split(&delim)
                .map(|s| Value::Literal(s.trim().to_string()))
                .collect()
        })
        .collect();

    self.stack.push(Value::Table { columns, rows });
    Ok(())
}
```

### 11.2 `into-columns` - Fixed-width auto-detect

```rust
fn builtin_into_columns(&mut self) -> Result<(), EvalError> {
    let text = self.pop_string()?;
    let lines: Vec<&str> = text.lines().collect();

    if lines.is_empty() {
        self.stack.push(Value::Table { columns: vec![], rows: vec![] });
        return Ok(());
    }

    // Detect column boundaries from header (2+ spaces)
    let header = lines[0];
    let mut boundaries = vec![0];
    let mut in_space = false;
    let mut space_start = 0;

    for (i, c) in header.char_indices() {
        if c == ' ' {
            if !in_space {
                in_space = true;
                space_start = i;
            }
        } else if in_space {
            if i - space_start >= 2 {
                boundaries.push(space_start);
            }
            in_space = false;
        }
    }
    boundaries.push(header.len());

    // Extract columns from boundaries
    let columns: Vec<String> = boundaries.windows(2)
        .map(|w| header[w[0]..w[1]].trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // Parse rows using same boundaries
    let rows: Vec<Vec<Value>> = lines[1..].iter()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            boundaries.windows(2)
                .map(|w| {
                    let start = w[0].min(line.len());
                    let end = w[1].min(line.len());
                    Value::Literal(line[start..end].trim().to_string())
                })
                .filter(|v| !matches!(v, Value::Literal(s) if s.is_empty()))
                .collect()
        })
        .collect();

    self.stack.push(Value::Table { columns, rows });
    Ok(())
}
```

---

## Phase 12: Trace Mode

### 12.1 Add `--trace` flag

In `main.rs`:

```rust
struct Args {
    // ... existing
    trace: bool,
}

// In clap/argument parsing:
.arg(Arg::new("trace")
    .long("trace")
    .help("Show stack state after each operation")
    .action(ArgAction::SetTrue))
```

### 12.2 Trace output in evaluator

Add `trace_enabled: bool` field to `Evaluator`.

In `eval_expr`, after each expression:

```rust
if self.trace_enabled {
    eprintln!(">>> {:?}", expr);
    eprintln!("    stack: {:?}", self.stack.iter()
        .map(format_value_short)
        .collect::<Vec<_>>());
}
```

---

## Implementation Order

1. **Phase 5**: `peek`, `tap`, `dip` (small, high value)
2. **Phase 6**: `sum`, `avg`, `min`, `max`, `count`
3. **Phase 7**: Deep path access in `get`
4. **Phase 8**: `group-by`, `add-column`, `unique`, `reverse`
5. **Phase 9**: Format-on-display (`src/display.rs`)
6. **Phase 10**: `fetch` (requires ureq dependency)
7. **Phase 11**: `into-tsv`, `into-columns`
8. **Phase 12**: `--trace` mode

---

## Test Coverage

Each feature gets tests:

```rust
// Phase 5
#[test] fn test_peek_single()
#[test] fn test_peek_multiple()
#[test] fn test_tap_side_effect()
#[test] fn test_dip_operates_under()

// Phase 6
#[test] fn test_sum_list()
#[test] fn test_avg_list()
#[test] fn test_min_max()
#[test] fn test_count_list()
#[test] fn test_count_table()

// Phase 7
#[test] fn test_deep_get()
#[test] fn test_deep_get_array_index()

// Phase 8
#[test] fn test_group_by()
#[test] fn test_add_column()
#[test] fn test_unique_list()
#[test] fn test_reverse_list()

// Phase 9
#[test] fn test_format_table()
#[test] fn test_format_record()

// Phase 10
#[test] fn test_fetch_url() // may need mock or skip in CI

// Phase 11
#[test] fn test_into_tsv()
#[test] fn test_into_tsv_custom_delim()
#[test] fn test_into_columns()

// Phase 12
#[test] fn test_trace_mode()
```

---

## Files Summary

| File | Changes |
|------|---------|
| `src/eval.rs` | All new builtins |
| `src/resolver.rs` | Add builtins to list |
| `src/display.rs` | **NEW** - Format-on-display |
| `src/main.rs` | --trace flag, display integration |
| `Cargo.toml` | Add ureq dependency |
| `tests/integration_tests.rs` | All new tests |
