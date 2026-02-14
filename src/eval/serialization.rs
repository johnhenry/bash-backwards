use super::{Evaluator, EvalError};
use crate::ast::Value;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::PathBuf;

impl Evaluator {
    pub(crate) fn json_parse(&mut self) -> Result<(), EvalError> {
        let s = self.pop_string()?;
        let json: JsonValue = serde_json::from_str(&s)
            .map_err(|e| EvalError::ExecError(format!("JSON parse error: {}", e)))?;
        let value = crate::ast::json_to_value(json);
        self.stack.push(value);
        Ok(())
    }

    pub(crate) fn json_stringify(&mut self) -> Result<(), EvalError> {
        let value = self.pop_value_or_err()?;
        let json = crate::ast::value_to_json(&value);
        let output = serde_json::to_string_pretty(&json)
            .map_err(|e| EvalError::ExecError(format!("JSON error: {}", e)))?;
        self.stack.push(Value::Output(output));
        Ok(())
    }

    pub(crate) fn builtin_into_json(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("into-json requires string".into()))?;
        let text = val.as_arg().ok_or_else(||
            EvalError::TypeError { expected: "String".into(), got: format!("{:?}", val) })?;

        let json: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| EvalError::ExecError(format!("into-json: {}", e)))?;

        self.stack.push(crate::ast::json_to_value(json));
        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_into_csv(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("into-csv requires string".into()))?;
        let text = val.as_arg().ok_or_else(||
            EvalError::TypeError { expected: "String".into(), got: format!("{:?}", val) })?;

        let mut lines = text.lines();
        let header = lines.next().ok_or_else(||
            EvalError::ExecError("into-csv: empty input".into()))?;

        let columns: Vec<String> = header.split(',')
            .map(|s| s.trim().to_string())
            .collect();

        let rows: Vec<Vec<Value>> = lines
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                line.split(',')
                    .map(|s| {
                        let trimmed = s.trim();
                        // Try to parse as number
                        if let Ok(n) = trimmed.parse::<f64>() {
                            Value::Number(n)
                        } else {
                            Value::Literal(trimmed.to_string())
                        }
                    })
                    .collect()
            })
            .collect();

        self.stack.push(Value::Table { columns, rows });
        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_into_lines(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("into-lines requires string".into()))?;
        let text = val.as_arg().ok_or_else(||
            EvalError::TypeError { expected: "String".into(), got: format!("{:?}", val) })?;

        let lines: Vec<Value> = text.lines()
            .map(|s| Value::Literal(s.to_string()))
            .collect();

        self.stack.push(Value::List(lines));
        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_into_kv(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("into-kv requires string".into()))?;
        let text = val.as_arg().ok_or_else(||
            EvalError::TypeError { expected: "String".into(), got: format!("{:?}", val) })?;

        let mut map = HashMap::new();
        for line in text.lines() {
            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim().to_string();
                let value = line[eq_pos + 1..].trim().to_string();
                map.insert(key, Value::Literal(value));
            }
        }

        self.stack.push(Value::Map(map));
        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_into_tsv(&mut self) -> Result<(), EvalError> {
        let text = self.pop_string()?;
        self.parse_delimited_text(&text, "\t")
    }

    pub(crate) fn builtin_into_delimited(&mut self) -> Result<(), EvalError> {
        let delim = self.pop_string()?;
        let text = self.pop_string()?;
        self.parse_delimited_text(&text, &delim)
    }

    pub(crate) fn parse_delimited_text(&mut self, text: &str, delim: &str) -> Result<(), EvalError> {
        let lines: Vec<&str> = text.lines().collect();
        if lines.is_empty() {
            self.stack.push(Value::Table { columns: vec![], rows: vec![] });
            return Ok(());
        }

        let columns: Vec<String> = lines[0].split(delim)
            .map(|s| s.trim().to_string())
            .collect();

        let rows: Vec<Vec<Value>> = lines[1..].iter()
            .filter(|l| !l.trim().is_empty())
            .map(|line| {
                line.split(delim)
                    .map(|s| Value::Literal(s.trim().to_string()))
                    .collect()
            })
            .collect();

        self.stack.push(Value::Table { columns, rows });
        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_to_json(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("to-json requires value".into()))?;

        let json = crate::ast::value_to_json(&val);
        let text = serde_json::to_string(&json)
            .map_err(|e| EvalError::ExecError(format!("to-json: {}", e)))?;

        self.stack.push(Value::Output(text));
        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_to_csv(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("to-csv requires table".into()))?;

        match val {
            Value::Table { columns, rows } => {
                let mut lines = vec![columns.join(",")];
                for row in rows {
                    let line: Vec<String> = row.iter()
                        .map(|v| v.as_arg().unwrap_or_default())
                        .collect();
                    lines.push(line.join(","));
                }
                self.stack.push(Value::Output(lines.join("\n")));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Table".into(),
                got: format!("{:?}", val),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_to_lines(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("to-lines requires list".into()))?;

        match val {
            Value::List(items) => {
                let lines: Vec<String> = items.iter()
                    .filter_map(|v| v.as_arg())
                    .collect();
                self.stack.push(Value::Output(lines.join("\n")));
            }
            _ => return Err(EvalError::TypeError {
                expected: "List".into(),
                got: format!("{:?}", val),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    pub(crate) fn builtin_to_kv(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("to-kv requires record".into()))?;

        match val {
            Value::Map(map) => {
                let mut pairs: Vec<_> = map.iter()
                    .map(|(k, v)| {
                        let val_str = v.as_arg().unwrap_or_default();
                        format!("{}={}", k, val_str)
                    })
                    .collect();
                pairs.sort(); // Consistent ordering
                self.stack.push(Value::Output(pairs.join("\n")));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Record".into(),
                got: format!("{:?}", val),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    /// to-tsv: Convert table to TSV string format
    pub(crate) fn builtin_to_tsv(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("to-tsv requires table".into()))?;

        match val {
            Value::Table { columns, rows } => {
                let mut lines = vec![columns.join("\t")];
                for row in rows {
                    let line: Vec<String> = row.iter()
                        .map(|v| v.as_arg().unwrap_or_default())
                        .collect();
                    lines.push(line.join("\t"));
                }
                self.stack.push(Value::Output(lines.join("\n")));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Table".into(),
                got: format!("{:?}", val),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    /// to-delimited: Convert table to custom-delimited string format
    /// table delimiter to-delimited -> delimited string
    pub(crate) fn builtin_to_delimited(&mut self) -> Result<(), EvalError> {
        let delim_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("to-delimited requires delimiter".into()))?;
        let table_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("to-delimited requires table".into()))?;

        let delim = delim_val.as_arg().unwrap_or_else(|| ",".to_string());

        match table_val {
            Value::Table { columns, rows } => {
                let mut lines = vec![columns.join(&delim)];
                for row in rows {
                    let line: Vec<String> = row.iter()
                        .map(|v| v.as_arg().unwrap_or_default())
                        .collect();
                    lines.push(line.join(&delim));
                }
                self.stack.push(Value::Output(lines.join("\n")));
            }
            _ => return Err(EvalError::TypeError {
                expected: "Table".into(),
                got: format!("{:?}", table_val),
            }),
        }

        self.last_exit_code = 0;
        Ok(())
    }

    /// open: Open a file and parse it based on extension
    /// Supports: .json, .csv, .tsv, plain text
    pub(crate) fn builtin_open(&mut self) -> Result<(), EvalError> {
        use std::fs;

        let path_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("open requires file path".into()))?;
        let path_str = path_val.as_arg().ok_or_else(||
            EvalError::TypeError { expected: "String".into(), got: format!("{:?}", path_val) })?;
        let path = PathBuf::from(self.expand_tilde(&path_str));

        let content = fs::read_to_string(&path).map_err(|e| {
            EvalError::IoError(std::io::Error::new(e.kind(), format!("{}: {}", path.display(), e)))
        })?;

        // Determine format based on extension
        let ext = path.extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        match ext.as_str() {
            "json" => {
                // Parse as JSON
                self.stack.push(Value::Literal(content));
                self.json_parse()?;
            }
            "csv" => {
                // Parse as CSV
                self.stack.push(Value::Literal(content));
                self.builtin_into_csv()?;
            }
            "tsv" => {
                // Parse as TSV
                self.stack.push(Value::Literal(content));
                self.builtin_into_tsv()?;
            }
            _ => {
                // Plain text - just push as output
                self.stack.push(Value::Output(content));
            }
        }

        self.last_exit_code = 0;
        Ok(())
    }

    /// save: Write data to file, auto-formatting based on extension
    /// data "path.json" save -> writes JSON
    /// data "path.csv" save -> writes CSV
    pub(crate) fn builtin_save(&mut self) -> Result<(), EvalError> {
        use std::fs;

        let path_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("save requires file path".into()))?;
        let data_val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("save requires data".into()))?;

        let path_str = path_val.as_arg().ok_or_else(||
            EvalError::TypeError { expected: "String".into(), got: format!("{:?}", path_val) })?;
        let path = PathBuf::from(self.expand_tilde(&path_str));

        // Determine format based on extension
        let ext = path.extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        let content = match ext.as_str() {
            "json" => {
                // Convert to JSON
                let json = crate::ast::value_to_json(&data_val);
                serde_json::to_string_pretty(&json)
                    .unwrap_or_else(|_| data_val.as_arg().unwrap_or_default())
            }
            "csv" => {
                // Convert to CSV
                match &data_val {
                    Value::Table { columns, rows } => {
                        let mut lines = vec![columns.join(",")];
                        for row in rows {
                            let line: Vec<String> = row.iter()
                                .map(|v| v.as_arg().unwrap_or_default())
                                .collect();
                            lines.push(line.join(","));
                        }
                        lines.join("\n")
                    }
                    _ => data_val.as_arg().unwrap_or_default(),
                }
            }
            "tsv" => {
                // Convert to TSV
                match &data_val {
                    Value::Table { columns, rows } => {
                        let mut lines = vec![columns.join("\t")];
                        for row in rows {
                            let line: Vec<String> = row.iter()
                                .map(|v| v.as_arg().unwrap_or_default())
                                .collect();
                            lines.push(line.join("\t"));
                        }
                        lines.join("\n")
                    }
                    _ => data_val.as_arg().unwrap_or_default(),
                }
            }
            _ => {
                // Plain text
                data_val.as_arg().unwrap_or_default()
            }
        };

        fs::write(&path, content).map_err(|e| {
            EvalError::IoError(std::io::Error::new(e.kind(), format!("{}: {}", path.display(), e)))
        })?;

        self.last_exit_code = 0;
        Ok(())
    }
}
