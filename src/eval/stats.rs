use super::{Evaluator, EvalError};
use crate::ast::Value;
use std::collections::HashMap;

/// Helper to extract a Vec<f64> from a Value::List
fn extract_numbers(val: &Value, _op: &str) -> Result<Vec<f64>, EvalError> {
    match val {
        Value::List(items) => {
            let nums: Vec<f64> = items.iter().filter_map(|v| match v {
                Value::Number(n) => Some(*n),
                Value::Literal(s) | Value::Output(s) => s.trim().parse().ok(),
                _ => None,
            }).collect();
            Ok(nums)
        }
        _ => Err(EvalError::TypeError {
            expected: "List".into(),
            got: format!("{:?}", val),
        }),
    }
}

impl Evaluator {
    /// product: Multiply all elements in a list
    /// [nums] product -> number
    pub(crate) fn builtin_product(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("product requires a list".into()))?;
        let nums = extract_numbers(&val, "product")?;
        let result = nums.iter().fold(1.0, |acc, &x| acc * x);
        self.stack.push(Value::Number(result));
        self.last_exit_code = 0;
        Ok(())
    }

    /// median: Middle value of sorted list
    /// [nums] median -> number
    pub(crate) fn builtin_median(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("median requires a list".into()))?;
        let mut nums = extract_numbers(&val, "median")?;
        if nums.is_empty() {
            self.stack.push(Value::Nil);
            self.last_exit_code = 0;
            return Ok(());
        }
        nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let len = nums.len();
        let result = if len % 2 == 0 {
            (nums[len / 2 - 1] + nums[len / 2]) / 2.0
        } else {
            nums[len / 2]
        };
        self.stack.push(Value::Number(result));
        self.last_exit_code = 0;
        Ok(())
    }

    /// mode: Most frequently occurring value
    /// [nums] mode -> number
    pub(crate) fn builtin_mode(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("mode requires a list".into()))?;
        let nums = extract_numbers(&val, "mode")?;
        if nums.is_empty() {
            self.stack.push(Value::Nil);
            self.last_exit_code = 0;
            return Ok(());
        }
        let mut counts: HashMap<String, (f64, usize)> = HashMap::new();
        for &n in &nums {
            let key = format!("{}", n);
            let entry = counts.entry(key).or_insert((n, 0));
            entry.1 += 1;
        }
        let (mode_val, _) = counts.values()
            .max_by_key(|(_, count)| *count)
            .unwrap();
        self.stack.push(Value::Number(*mode_val));
        self.last_exit_code = 0;
        Ok(())
    }

    /// modes: All values sharing the highest frequency
    /// [nums] modes -> [numbers]
    pub(crate) fn builtin_modes(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("modes requires a list".into()))?;
        let nums = extract_numbers(&val, "modes")?;
        if nums.is_empty() {
            self.stack.push(Value::List(vec![]));
            self.last_exit_code = 0;
            return Ok(());
        }
        let mut counts: HashMap<String, (f64, usize)> = HashMap::new();
        for &n in &nums {
            let key = format!("{}", n);
            let entry = counts.entry(key).or_insert((n, 0));
            entry.1 += 1;
        }
        let max_count = counts.values().map(|(_, c)| *c).max().unwrap_or(0);
        let modes: Vec<Value> = counts.values()
            .filter(|(_, c)| *c == max_count)
            .map(|(v, _)| Value::Number(*v))
            .collect();
        self.stack.push(Value::List(modes));
        self.last_exit_code = 0;
        Ok(())
    }

    /// variance: Population variance (divide by N)
    /// [nums] variance -> number
    pub(crate) fn builtin_variance(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("variance requires a list".into()))?;
        let nums = extract_numbers(&val, "variance")?;
        if nums.is_empty() {
            self.stack.push(Value::Number(0.0));
            self.last_exit_code = 0;
            return Ok(());
        }
        let mean = nums.iter().sum::<f64>() / nums.len() as f64;
        let variance = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / nums.len() as f64;
        self.stack.push(Value::Number(variance));
        self.last_exit_code = 0;
        Ok(())
    }

    /// sample-variance: Sample variance (divide by N-1, Bessel's correction)
    /// [nums] sample-variance -> number
    pub(crate) fn builtin_sample_variance(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("sample-variance requires a list".into()))?;
        let nums = extract_numbers(&val, "sample-variance")?;
        if nums.len() < 2 {
            self.stack.push(Value::Number(0.0));
            self.last_exit_code = 0;
            return Ok(());
        }
        let mean = nums.iter().sum::<f64>() / nums.len() as f64;
        let variance = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (nums.len() - 1) as f64;
        self.stack.push(Value::Number(variance));
        self.last_exit_code = 0;
        Ok(())
    }

    /// stdev: Population standard deviation
    /// [nums] stdev -> number
    pub(crate) fn builtin_stdev(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("stdev requires a list".into()))?;
        let nums = extract_numbers(&val, "stdev")?;
        if nums.is_empty() {
            self.stack.push(Value::Number(0.0));
            self.last_exit_code = 0;
            return Ok(());
        }
        let mean = nums.iter().sum::<f64>() / nums.len() as f64;
        let variance = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / nums.len() as f64;
        self.stack.push(Value::Number(variance.sqrt()));
        self.last_exit_code = 0;
        Ok(())
    }

    /// sample-stdev: Sample standard deviation (uses N-1)
    /// [nums] sample-stdev -> number
    pub(crate) fn builtin_sample_stdev(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("sample-stdev requires a list".into()))?;
        let nums = extract_numbers(&val, "sample-stdev")?;
        if nums.len() < 2 {
            self.stack.push(Value::Number(0.0));
            self.last_exit_code = 0;
            return Ok(());
        }
        let mean = nums.iter().sum::<f64>() / nums.len() as f64;
        let variance = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (nums.len() - 1) as f64;
        self.stack.push(Value::Number(variance.sqrt()));
        self.last_exit_code = 0;
        Ok(())
    }

    /// percentile: Value at given percentile using linear interpolation
    /// [nums] 0.5 percentile -> number (0.5 = 50th percentile = median)
    pub(crate) fn builtin_percentile(&mut self) -> Result<(), EvalError> {
        let p = self.pop_number("percentile")?;
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("percentile requires a list".into()))?;
        let mut nums = extract_numbers(&val, "percentile")?;
        if nums.is_empty() {
            self.stack.push(Value::Nil);
            self.last_exit_code = 0;
            return Ok(());
        }
        if p < 0.0 || p > 1.0 {
            return Err(EvalError::ExecError(
                format!("percentile: p must be between 0.0 and 1.0, got {}", p)
            ));
        }
        nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let pos = p * (nums.len() - 1) as f64;
        let lower = pos.floor() as usize;
        let upper = pos.ceil() as usize;
        let result = if lower == upper {
            nums[lower]
        } else {
            let frac = pos - lower as f64;
            nums[lower] * (1.0 - frac) + nums[upper] * frac
        };
        self.stack.push(Value::Number(result));
        self.last_exit_code = 0;
        Ok(())
    }

    /// five-num: Five-number summary [min, Q1, median, Q3, max]
    /// [nums] five-num -> [min, Q1, median, Q3, max]
    pub(crate) fn builtin_five_num(&mut self) -> Result<(), EvalError> {
        let val = self.stack.pop().ok_or_else(||
            EvalError::StackUnderflow("five-num requires a list".into()))?;
        let mut nums = extract_numbers(&val, "five-num")?;
        if nums.is_empty() {
            self.stack.push(Value::List(vec![]));
            self.last_exit_code = 0;
            return Ok(());
        }
        nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let len = nums.len();
        let min_val = nums[0];
        let max_val = nums[len - 1];

        // Median
        let median = if len % 2 == 0 {
            (nums[len / 2 - 1] + nums[len / 2]) / 2.0
        } else {
            nums[len / 2]
        };

        // Q1: percentile at 0.25
        let q1_pos = 0.25 * (len - 1) as f64;
        let q1_lower = q1_pos.floor() as usize;
        let q1_upper = q1_pos.ceil() as usize;
        let q1 = if q1_lower == q1_upper {
            nums[q1_lower]
        } else {
            let frac = q1_pos - q1_lower as f64;
            nums[q1_lower] * (1.0 - frac) + nums[q1_upper] * frac
        };

        // Q3: percentile at 0.75
        let q3_pos = 0.75 * (len - 1) as f64;
        let q3_lower = q3_pos.floor() as usize;
        let q3_upper = q3_pos.ceil() as usize;
        let q3 = if q3_lower == q3_upper {
            nums[q3_lower]
        } else {
            let frac = q3_pos - q3_lower as f64;
            nums[q3_lower] * (1.0 - frac) + nums[q3_upper] * frac
        };

        self.stack.push(Value::List(vec![
            Value::Number(min_val),
            Value::Number(q1),
            Value::Number(median),
            Value::Number(q3),
            Value::Number(max_val),
        ]));
        self.last_exit_code = 0;
        Ok(())
    }
}
