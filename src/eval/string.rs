use super::{Evaluator, EvalError};
use crate::ast::Value;

impl Evaluator {
    /// Split at first occurrence of delimiter
    /// "a.b.c" "." split1 -> "a", "b.c"
    /// If not found: "abc" "." split1 -> "abc", ""
    pub(crate) fn string_split1(&mut self) -> Result<(), EvalError> {
        let delim = self.pop_string()?;
        let s = self.pop_string()?;

        match s.find(&delim) {
            Some(idx) => {
                let (left, right) = s.split_at(idx);
                self.stack.push(Value::Literal(left.to_string()));
                self.stack
                    .push(Value::Literal(right[delim.len()..].to_string()));
            }
            None => {
                self.stack.push(Value::Literal(s));
                self.stack.push(Value::Literal(String::new()));
            }
        }
        Ok(())
    }

    /// Split at last occurrence of delimiter
    /// "a.b.c" "." rsplit1 -> "a.b", "c"
    /// If not found: "abc" "." rsplit1 -> "", "abc"
    pub(crate) fn string_rsplit1(&mut self) -> Result<(), EvalError> {
        let delim = self.pop_string()?;
        let s = self.pop_string()?;

        match s.rfind(&delim) {
            Some(idx) => {
                let (left, right) = s.split_at(idx);
                self.stack.push(Value::Literal(left.to_string()));
                self.stack
                    .push(Value::Literal(right[delim.len()..].to_string()));
            }
            None => {
                self.stack.push(Value::Literal(String::new()));
                self.stack.push(Value::Literal(s));
            }
        }
        Ok(())
    }

    /// Get string length
    /// Usage: "hello" len -> 5
    pub(crate) fn builtin_len(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            return Err(EvalError::ExecError("len: string required".into()));
        }
        self.restore_excess_args(args, 1);
        let s = &args[0];
        self.stack.push(Value::Output(s.chars().count().to_string()));
        self.last_exit_code = 0;
        Ok(())
    }

    /// Extract substring
    /// Usage: "hello" 1 3 slice -> "ell" (start at index 1, take 3 chars)
    pub(crate) fn builtin_slice(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.len() < 3 {
            return Err(EvalError::ExecError("slice: string start length required".into()));
        }
        self.restore_excess_args(args, 3);
        // Args in LIFO: [length, start, string] for "string start length slice"
        let length: usize = args[0].parse().unwrap_or(0);
        let start: usize = args[1].parse().unwrap_or(0);
        let s = &args[2];

        let result: String = s.chars().skip(start).take(length).collect();
        self.stack.push(Value::Output(result));
        self.last_exit_code = 0;
        Ok(())
    }

    /// Find substring, returns index or -1 if not found
    /// Usage: "hello" "ll" indexof -> 2
    pub(crate) fn builtin_indexof(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.len() < 2 {
            return Err(EvalError::ExecError("indexof: string needle required".into()));
        }
        self.restore_excess_args(args, 2);
        // Args in LIFO: [needle, haystack] for "haystack needle indexof"
        let needle = &args[0];
        let haystack = &args[1];

        let result = match haystack.find(needle.as_str()) {
            Some(idx) => idx as i64,
            None => -1,
        };
        self.stack.push(Value::Output(result.to_string()));
        self.last_exit_code = 0;
        Ok(())
    }

    /// Replace all occurrences of a substring
    /// Usage: "hello" "l" "L" str-replace -> "heLLo"
    pub(crate) fn builtin_str_replace(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.len() < 3 {
            return Err(EvalError::ExecError("str-replace: string from to required".into()));
        }
        self.restore_excess_args(args, 3);
        // Args in LIFO: [to, from, string] for "string from to str-replace"
        let to = &args[0];
        let from = &args[1];
        let s = &args[2];

        let result = s.replace(from, to);
        self.stack.push(Value::Output(result));
        self.last_exit_code = 0;
        Ok(())
    }

    /// String interpolation: name "Hello, {}!" format -> "Hello, Alice!"
    /// Positional: alice bob "{1} meets {0}" format -> "alice meets bob"
    pub(crate) fn builtin_format(&mut self, args: &[String]) -> Result<(), EvalError> {
        if args.is_empty() {
            return Err(EvalError::ExecError("format: template string required".into()));
        }

        // Convention: value1 value2 template format (template pushed LAST, just before format)
        // For Alice "Hello, {}!" format:
        //   Stack = ["Alice", "Hello, {}!"], pops -> args = ["Hello, {}!", "Alice"]
        // Template is FIRST in args (last pushed = top of stack = first popped)
        let template = &args[0];
        // Values are the rest, already in push order after reversing
        let values: Vec<&str> = args[1..].iter().rev().map(|s| s.as_str()).collect();

        let mut result = template.clone();
        let mut next_idx = 0;

        // Replace {} with next value
        while let Some(pos) = result.find("{}") {
            if next_idx >= values.len() {
                break;
            }
            result = format!("{}{}{}", &result[..pos], values[next_idx], &result[pos + 2..]);
            next_idx += 1;
        }

        // Replace {0}, {1}, etc. with positional values
        for (i, val) in values.iter().enumerate() {
            let placeholder = format!("{{{}}}", i);
            result = result.replace(&placeholder, val);
        }

        self.stack.push(Value::Output(result));
        self.last_exit_code = 0;
        Ok(())
    }
}
