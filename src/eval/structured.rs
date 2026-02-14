use super::{Evaluator, EvalError};
use crate::ast::{Expr, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

impl Evaluator {
    pub(crate) fn builtin_typeof(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("typeof requires a value".into()))?;

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
            Value::Media { .. } => "Media",
            Value::Link { .. } => "Link",
            Value::Bytes(_) => "Bytes",
            Value::BigInt(_) => "BigInt",
        };

        self.stack.push(Value::Literal(type_name.to_string()));
        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_record(&mut self) -> Result<(), EvalError> {
        let mut pairs: Vec<(String, Value)> = Vec::new();

        while self.stack.len() >= 2 {
            if matches!(self.stack.last(), Some(Value::Marker)) {
                self.stack.pop();
                break;
            }

            let potential_key = self.stack.get(self.stack.len() - 2);
            match potential_key {
                Some(Value::Literal(_)) | Some(Value::Output(_)) => {}
                _ => { break; }
            }

            let value = self.stack.pop().unwrap();
            let key_val = self.stack.pop().unwrap();
            let key = key_val.as_arg().unwrap();
            pairs.push((key, value));
        }

        if matches!(self.stack.last(), Some(Value::Marker)) {
            self.stack.pop();
        }

        pairs.reverse();
        let map: HashMap<String, Value> = pairs.into_iter().collect();
        self.stack.push(Value::Map(map));
        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_get(&mut self) -> Result<(), EvalError> {
        let key_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("get requires key".into()))?;
        let key = key_val.as_arg().ok_or_else(||
            EvalError::TypeError { expected: "String".into(), got: format!("{:?}", key_val) })?;

        let target = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("get requires record/table".into()))?;

        if key.contains('.') {
            let result = self.deep_get(&target, &key);
            self.stack.push(result);
            self.last_exit_code = 0;
            return Ok(());
        }

        match target {
            Value::Map(map) => {
                match map.get(&key) {
                    Some(v) => self.stack.push(v.clone()),
                    None => self.stack.push(Value::Nil),
                }
            }
            Value::List(items) => {
                if let Ok(idx) = key.parse::<usize>() {
                    self.stack.push(items.get(idx).cloned().unwrap_or(Value::Nil));
                } else {
                    self.stack.push(Value::Nil);
                }
            }
            Value::Table { columns, rows } => {
                if let Some(col_idx) = columns.iter().position(|c| c == &key) {
                    let values: Vec<Value> = rows.iter()
                        .map(|row| row.get(col_idx).cloned().unwrap_or(Value::Nil))
                        .collect();
                    self.stack.push(Value::List(values));
                } else {
                    self.stack.push(Value::Nil);
                }
            }
            Value::Error { kind, message, code, source, command } => {
                let field = match key.as_str() {
                    "kind" => Some(Value::Literal(kind)),
                    "message" => Some(Value::Literal(message)),
                    "code" => code.map(|c| Value::Number(c as f64)),
                    "source" => source.map(Value::Literal),
                    "command" => command.map(Value::Literal),
                    _ => None,
                };
                self.stack.push(field.unwrap_or(Value::Nil));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Record, Table, List, or Error".into(),
                got: format!("{:?}", target),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_set(&mut self) -> Result<(), EvalError> {
        let value = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("set requires value".into()))?;
        let key_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("set requires key".into()))?;
        let key = key_val.as_arg().ok_or_else(||
            EvalError::TypeError { expected: "String".into(), got: format!("{:?}", key_val) })?;
        let target = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("set requires record".into()))?;

        if key.contains('.') {
            let result = self.deep_set(target, &key, value)?;
            self.stack.push(result);
            self.last_exit_code = 0;
            return Ok(());
        }

        match target {
            Value::Map(mut map) => {
                map.insert(key, value);
                self.stack.push(Value::Map(map));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Record".into(),
                got: format!("{:?}", target),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_del(&mut self) -> Result<(), EvalError> {
        let key_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("del requires key".into()))?;
        let key = key_val.as_arg().ok_or_else(||
            EvalError::TypeError { expected: "String".into(), got: format!("{:?}", key_val) })?;
        let target = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("del requires record".into()))?;

        match target {
            Value::Map(mut map) => {
                map.remove(&key);
                self.stack.push(Value::Map(map));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Record".into(),
                got: format!("{:?}", target),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_has(&mut self) -> Result<(), EvalError> {
        let key_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("has? requires key".into()))?;
        let key = key_val.as_arg().ok_or_else(||
            EvalError::TypeError { expected: "String".into(), got: format!("{:?}", key_val) })?;
        let target = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("has? requires record".into()))?;

        let has_key = match target {
            Value::Map(map) => map.contains_key(&key),
            Value::Table { columns, .. } => columns.contains(&key),
            _ => false,
        };

        self.last_exit_code = if has_key { 0 } else { 1 };
        Ok(())
    }

    pub(crate) fn builtin_keys(&mut self) -> Result<(), EvalError> {
        let target = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("keys requires record".into()))?;

        match target {
            Value::Map(map) => {
                let keys: Vec<Value> = map.keys()
                    .map(|k| Value::Literal(k.clone()))
                    .collect();
                self.stack.push(Value::List(keys));
            }
            Value::Table { columns, .. } => {
                let keys: Vec<Value> = columns.iter()
                    .map(|k| Value::Literal(k.clone()))
                    .collect();
                self.stack.push(Value::List(keys));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Record or Table".into(),
                got: format!("{:?}", target),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_values(&mut self) -> Result<(), EvalError> {
        let target = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("values requires record".into()))?;

        match target {
            Value::Map(map) => {
                let values: Vec<Value> = map.values().cloned().collect();
                self.stack.push(Value::List(values));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Record".into(),
                got: format!("{:?}", target),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_merge(&mut self) -> Result<(), EvalError> {
        let right = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("merge requires two records".into()))?;
        let left = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("merge requires two records".into()))?;

        match (left, right) {
            (Value::Map(mut left_map), Value::Map(right_map)) => {
                left_map.extend(right_map);
                self.stack.push(Value::Map(left_map));
            }
            _ => return Err(EvalError::TypeError {
                expected: "two Records".into(),
                got: "non-record values".into(),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_table(&mut self) -> Result<(), EvalError> {
        let mut records: Vec<HashMap<String, Value>> = Vec::new();

        while let Some(val) = self.stack.pop() {
            match val {
                Value::Marker => break,
                Value::Map(map) => records.push(map),
                _ => return Err(EvalError::TypeError {
                    expected: "Record".into(),
                    got: format!("{:?}", val),
                }),
            }
        }

        records.reverse();

        if records.is_empty() {
            self.stack.push(Value::Table { columns: vec![], rows: vec![] });
            self.last_exit_code = 0;
            return Ok(());
        }

        let columns: Vec<String> = records[0].keys().cloned().collect();

        let rows: Vec<Vec<Value>> = records.iter()
            .map(|rec| {
                columns.iter()
                    .map(|col| rec.get(col).cloned().unwrap_or(Value::Nil))
                    .collect()
            })
            .collect();

        self.stack.push(Value::Table { columns, rows });
        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_where(&mut self) -> Result<(), EvalError> {
        let predicate = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("where requires predicate block".into()))?;
        let pred_block = match predicate {
            Value::Block(exprs) => exprs,
            _ => return Err(EvalError::TypeError {
                expected: "Block".into(),
                got: format!("{:?}", predicate),
            }),
        };

        let table = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("where requires table".into()))?;

        match table {
            Value::Table { columns, rows } => {
                let mut filtered_rows = Vec::new();

                for row in rows {
                    let record: HashMap<String, Value> = columns.iter()
                        .zip(row.iter())
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();

                    let saved_stack = std::mem::take(&mut self.stack);
                    self.stack.push(Value::Map(record.clone()));

                    for expr in &pred_block {
                        self.eval_expr(expr)?;
                    }

                    let keep = self.last_exit_code == 0;
                    self.stack = saved_stack;

                    if keep {
                        filtered_rows.push(row);
                    }
                }

                self.stack.push(Value::Table { columns, rows: filtered_rows });
            }
            _ => return Err(EvalError::TypeError {
                expected: "Table".into(),
                got: format!("{:?}", table),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_sort_by(&mut self) -> Result<(), EvalError> {
        let key_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("sort-by requires key/column".into()))?;

        let data = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("sort-by requires table or list".into()))?;

        match data {
            Value::Table { columns, mut rows } => {
                let col = key_val.as_arg().ok_or_else(||
                    EvalError::TypeError { expected: "String".into(), got: format!("{:?}", key_val) })?;

                if let Some(col_idx) = columns.iter().position(|c| c == &col) {
                    rows.sort_by(|a, b| {
                        let av = a.get(col_idx).and_then(|v| v.as_arg()).unwrap_or_default();
                        let bv = b.get(col_idx).and_then(|v| v.as_arg()).unwrap_or_default();
                        Self::compare_strings(&av, &bv)
                    });
                }
                self.stack.push(Value::Table { columns, rows });
            }
            Value::List(mut items) => {
                let key_name = key_val.as_arg().unwrap_or_default();

                items.sort_by(|a, b| {
                    let av = Self::extract_sort_key(a, &key_name);
                    let bv = Self::extract_sort_key(b, &key_name);
                    Self::compare_strings(&av, &bv)
                });

                self.stack.push(Value::List(items));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Table or List".into(),
                got: format!("{:?}", data),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn extract_sort_key(val: &Value, key: &str) -> String {
        match val {
            Value::Map(m) => m.get(key)
                .and_then(|v| v.as_arg())
                .unwrap_or_default(),
            _ => val.as_arg().unwrap_or_default(),
        }
    }

    pub(crate) fn compare_strings(a: &str, b: &str) -> std::cmp::Ordering {
        match (a.parse::<f64>(), b.parse::<f64>()) {
            (Ok(an), Ok(bn)) => an.partial_cmp(&bn).unwrap_or(std::cmp::Ordering::Equal),
            _ => a.cmp(b),
        }
    }

    pub(crate) fn builtin_select(&mut self) -> Result<(), EvalError> {
        let cols_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("select requires column list".into()))?;

        let cols: Vec<String> = match cols_val {
            Value::List(items) => items.iter()
                .filter_map(|v| v.as_arg())
                .collect(),
            Value::Block(exprs) => {
                exprs.iter()
                    .filter_map(|e| match e {
                        Expr::Literal(s) => Some(s.clone()),
                        Expr::Quoted { content, .. } => Some(content.clone()),
                        _ => None,
                    })
                    .collect()
            }
            _ => return Err(EvalError::TypeError {
                expected: "List or Block".into(),
                got: format!("{:?}", cols_val),
            }),
        };

        let table = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("select requires table".into()))?;

        match table {
            Value::Table { columns, rows } => {
                let indices: Vec<usize> = cols.iter()
                    .filter_map(|c| columns.iter().position(|col| col == c))
                    .collect();

                let new_rows: Vec<Vec<Value>> = rows.iter()
                    .map(|row| {
                        indices.iter()
                            .map(|&i| row.get(i).cloned().unwrap_or(Value::Nil))
                            .collect()
                    })
                    .collect();

                self.stack.push(Value::Table { columns: cols, rows: new_rows });
            }
            _ => return Err(EvalError::TypeError {
                expected: "Table".into(),
                got: format!("{:?}", table),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_first(&mut self) -> Result<(), EvalError> {
        let n_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("first requires count".into()))?;
        let n: usize = n_val.as_arg()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);

        let table = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("first requires table".into()))?;

        match table {
            Value::Table { columns, rows } => {
                let new_rows: Vec<Vec<Value>> = rows.into_iter().take(n).collect();
                self.stack.push(Value::Table { columns, rows: new_rows });
            }
            Value::List(items) => {
                let new_items: Vec<Value> = items.into_iter().take(n).collect();
                self.stack.push(Value::List(new_items));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Table or List".into(),
                got: format!("{:?}", table),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_last(&mut self) -> Result<(), EvalError> {
        let n_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("last requires count".into()))?;
        let n: usize = n_val.as_arg()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);

        let table = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("last requires table".into()))?;

        match table {
            Value::Table { columns, rows } => {
                let len = rows.len();
                let skip = len.saturating_sub(n);
                let new_rows: Vec<Vec<Value>> = rows.into_iter().skip(skip).collect();
                self.stack.push(Value::Table { columns, rows: new_rows });
            }
            Value::List(items) => {
                let len = items.len();
                let skip = len.saturating_sub(n);
                let new_items: Vec<Value> = items.into_iter().skip(skip).collect();
                self.stack.push(Value::List(new_items));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Table or List".into(),
                got: format!("{:?}", table),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_nth(&mut self) -> Result<(), EvalError> {
        let n_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("nth requires index".into()))?;
        let n: usize = n_val.as_arg()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let table = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("nth requires table".into()))?;

        match table {
            Value::Table { columns, rows } => {
                if n < rows.len() {
                    let row = &rows[n];
                    let record: HashMap<String, Value> = columns.iter()
                        .zip(row.iter())
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();
                    self.stack.push(Value::Map(record));
                } else {
                    self.stack.push(Value::Nil);
                }
            }
            Value::List(items) => {
                if n < items.len() {
                    self.stack.push(items[n].clone());
                } else {
                    self.stack.push(Value::Nil);
                }
            }
            _ => return Err(EvalError::TypeError {
                expected: "Table or List".into(),
                got: format!("{:?}", table),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_try(&mut self) -> Result<(), EvalError> {
        let block = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("try requires block".into()))?;

        let exprs = match block {
            Value::Block(exprs) => exprs,
            _ => return Err(EvalError::TypeError {
                expected: "Block".into(),
                got: format!("{:?}", block),
            }),
        };

        let saved_stack = self.stack.clone();

        let result = (|| -> Result<(), EvalError> {
            for expr in &exprs {
                self.eval_expr(expr)?;
            }
            Ok(())
        })();

        match result {
            Ok(()) => {
                self.last_exit_code = 0;
            }
            Err(e) => {
                self.stack = saved_stack;
                self.stack.push(Value::Error {
                    kind: "eval_error".to_string(),
                    message: e.to_string(),
                    code: Some(self.last_exit_code),
                    source: None,
                    command: None,
                });
                self.last_exit_code = 1;
            }
        }

        Ok(())
    }

    pub(crate) fn builtin_error_predicate(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("error? requires value".into()))?;

        let is_error = matches!(val, Value::Error { .. });
        self.stack.push(val);

        self.last_exit_code = if is_error { 0 } else { 1 };
        Ok(())
    }

    pub(crate) fn builtin_throw(&mut self) -> Result<(), EvalError> {
        let msg_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("throw requires message".into()))?;
        let message = msg_val.as_arg().unwrap_or_else(|| "unknown error".to_string());

        self.stack.push(Value::Error {
            kind: "thrown".to_string(),
            message,
            code: None,
            source: None,
            command: None,
        });

        self.last_exit_code = 1;
        Ok(())
    }

    pub(crate) fn builtin_tap(&mut self) -> Result<(), EvalError> {
        let block = match self.stack.pop() {
            Some(Value::Block(b)) => b,
            Some(other) => return Err(EvalError::TypeError {
                expected: "Block".into(),
                got: format!("{:?}", other),
            }),
            None => return Err(EvalError::StackUnderflow("tap requires block".into())),
        };

        let original = self.stack.last().cloned()
            .ok_or_else(|| EvalError::StackUnderflow("tap requires a value under block".into()))?;

        let depth_before_original = self.stack.len() - 1;

        for expr in &block {
            self.eval_expr(expr)?;
        }

        self.stack.truncate(depth_before_original);
        self.stack.push(original);

        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_dip(&mut self) -> Result<(), EvalError> {
        let block = match self.stack.pop() {
            Some(Value::Block(b)) => b,
            Some(other) => return Err(EvalError::TypeError {
                expected: "Block".into(),
                got: format!("{:?}", other),
            }),
            None => return Err(EvalError::StackUnderflow("dip requires block".into())),
        };

        let saved = self.stack.pop()
            .ok_or_else(|| EvalError::StackUnderflow("dip requires a value under block".into()))?;

        for expr in &block {
            self.eval_expr(expr)?;
        }

        self.stack.push(saved);
        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_ls_table(&mut self) -> Result<(), EvalError> {
        use std::fs;
        use std::os::unix::fs::MetadataExt;

        let dir_path = if let Some(val) = self.stack.last() {
            if let Some(s) = val.as_arg() {
                if !s.starts_with('-') && (Path::new(&s).exists() || s.contains('/')) {
                    self.stack.pop();
                    PathBuf::from(self.expand_tilde(&s))
                } else {
                    self.cwd.clone()
                }
            } else {
                self.cwd.clone()
            }
        } else {
            self.cwd.clone()
        };

        let entries = fs::read_dir(&dir_path).map_err(|e| {
            EvalError::IoError(std::io::Error::new(e.kind(), format!("{}: {}", dir_path.display(), e)))
        })?;

        let columns = vec![
            "name".to_string(),
            "type".to_string(),
            "size".to_string(),
            "modified".to_string(),
        ];

        let mut rows: Vec<Vec<Value>> = Vec::new();

        for entry in entries {
            if let Ok(entry) = entry {
                let name = entry.file_name().to_string_lossy().to_string();
                let metadata = entry.metadata();

                let (file_type, size, modified) = if let Ok(meta) = &metadata {
                    let ft = if meta.is_dir() { "dir" } else if meta.is_file() { "file" } else { "other" };
                    let sz = meta.len();
                    let mod_time = meta.mtime();
                    (ft.to_string(), sz, mod_time)
                } else {
                    ("unknown".to_string(), 0, 0)
                };

                rows.push(vec![
                    Value::Literal(name),
                    Value::Literal(file_type),
                    Value::Number(size as f64),
                    Value::Number(modified as f64),
                ]);
            }
        }

        rows.sort_by(|a, b| {
            let name_a = a.first().and_then(|v| v.as_arg()).unwrap_or_default();
            let name_b = b.first().and_then(|v| v.as_arg()).unwrap_or_default();
            name_a.cmp(&name_b)
        });

        self.stack.push(Value::Table { columns, rows });
        self.last_exit_code = 0;
        Ok(())
    }
}
