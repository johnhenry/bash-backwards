use super::{Evaluator, EvalError};
use crate::ast::Value;
use num_bigint::BigUint;

impl Evaluator {
    /// Convert value to BigInt: "123" to-bigint -> BigInt
    /// Accepts: decimal string, hex string (0x...), Bytes
    pub(crate) fn builtin_to_bigint(&mut self) -> Result<(), EvalError> {
        let value = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("to-bigint requires a value on stack".to_string())
        })?;

        let bigint = match &value {
            Value::Bytes(data) => {
                // Convert bytes to BigInt (big-endian)
                BigUint::from_bytes_be(data)
            }
            Value::Number(n) => {
                if *n < 0.0 {
                    self.stack.push(value);
                    return Err(EvalError::ExecError("to-bigint: negative numbers not supported".to_string()));
                }
                BigUint::from(*n as u64)
            }
            Value::BigInt(n) => {
                // Already BigInt
                n.clone()
            }
            Value::Literal(s) | Value::Output(s) => {
                let s = s.trim();
                if s.starts_with("0x") || s.starts_with("0X") {
                    // Parse as hex
                    BigUint::parse_bytes(s[2..].as_bytes(), 16).ok_or_else(|| {
                        EvalError::ExecError(format!("Invalid hex: {}", s))
                    })?
                } else {
                    // Parse as decimal
                    BigUint::parse_bytes(s.as_bytes(), 10).ok_or_else(|| {
                        EvalError::ExecError(format!("Invalid decimal: {}", s))
                    })?
                }
            }
            _ => {
                self.stack.push(value);
                return Err(EvalError::ExecError("to-bigint requires number, string, or Bytes".to_string()));
            }
        };

        self.stack.push(Value::BigInt(bigint));
        self.last_exit_code = 0;
        Ok(())
    }

    /// BigInt addition: a b big-add -> a+b
    pub(crate) fn builtin_big_add(&mut self) -> Result<(), EvalError> {
        let b = self.pop_bigint("big-add")?;
        let a = self.pop_bigint("big-add")?;
        self.stack.push(Value::BigInt(a + b));
        self.last_exit_code = 0;
        Ok(())
    }

    /// BigInt subtraction: a b big-sub -> a-b
    pub(crate) fn builtin_big_sub(&mut self) -> Result<(), EvalError> {
        let b = self.pop_bigint("big-sub")?;
        let a = self.pop_bigint("big-sub")?;
        if a < b {
            return Err(EvalError::ExecError("big-sub: result would be negative".to_string()));
        }
        self.stack.push(Value::BigInt(a - b));
        self.last_exit_code = 0;
        Ok(())
    }

    /// BigInt multiplication: a b big-mul -> a*b
    pub(crate) fn builtin_big_mul(&mut self) -> Result<(), EvalError> {
        let b = self.pop_bigint("big-mul")?;
        let a = self.pop_bigint("big-mul")?;
        self.stack.push(Value::BigInt(a * b));
        self.last_exit_code = 0;
        Ok(())
    }

    /// BigInt division: a b big-div -> a/b
    pub(crate) fn builtin_big_div(&mut self) -> Result<(), EvalError> {
        let b = self.pop_bigint("big-div")?;
        if b == BigUint::ZERO {
            return Err(EvalError::ExecError("big-div: division by zero".to_string()));
        }
        let a = self.pop_bigint("big-div")?;
        self.stack.push(Value::BigInt(a / b));
        self.last_exit_code = 0;
        Ok(())
    }

    /// BigInt modulo: a b big-mod -> a%b
    pub(crate) fn builtin_big_mod(&mut self) -> Result<(), EvalError> {
        let b = self.pop_bigint("big-mod")?;
        if b == BigUint::ZERO {
            return Err(EvalError::ExecError("big-mod: division by zero".to_string()));
        }
        let a = self.pop_bigint("big-mod")?;
        self.stack.push(Value::BigInt(a % b));
        self.last_exit_code = 0;
        Ok(())
    }

    /// BigInt XOR: a b big-xor -> a^b
    pub(crate) fn builtin_big_xor(&mut self) -> Result<(), EvalError> {
        let b = self.pop_bigint("big-xor")?;
        let a = self.pop_bigint("big-xor")?;
        self.stack.push(Value::BigInt(a ^ b));
        self.last_exit_code = 0;
        Ok(())
    }

    /// BigInt AND: a b big-and -> a&b
    pub(crate) fn builtin_big_and(&mut self) -> Result<(), EvalError> {
        let b = self.pop_bigint("big-and")?;
        let a = self.pop_bigint("big-and")?;
        self.stack.push(Value::BigInt(a & b));
        self.last_exit_code = 0;
        Ok(())
    }

    /// BigInt OR: a b big-or -> a|b
    pub(crate) fn builtin_big_or(&mut self) -> Result<(), EvalError> {
        let b = self.pop_bigint("big-or")?;
        let a = self.pop_bigint("big-or")?;
        self.stack.push(Value::BigInt(a | b));
        self.last_exit_code = 0;
        Ok(())
    }

    /// BigInt equality test: a b big-eq? -> exit code 0 if equal
    pub(crate) fn builtin_big_eq(&mut self) -> Result<(), EvalError> {
        let b = self.pop_bigint("big-eq?")?;
        let a = self.pop_bigint("big-eq?")?;
        self.last_exit_code = if a == b { 0 } else { 1 };
        Ok(())
    }

    /// BigInt less than: a b big-lt? -> exit code 0 if a < b
    pub(crate) fn builtin_big_lt(&mut self) -> Result<(), EvalError> {
        let b = self.pop_bigint("big-lt?")?;
        let a = self.pop_bigint("big-lt?")?;
        self.last_exit_code = if a < b { 0 } else { 1 };
        Ok(())
    }

    /// BigInt greater than: a b big-gt? -> exit code 0 if a > b
    pub(crate) fn builtin_big_gt(&mut self) -> Result<(), EvalError> {
        let b = self.pop_bigint("big-gt?")?;
        let a = self.pop_bigint("big-gt?")?;
        self.last_exit_code = if a > b { 0 } else { 1 };
        Ok(())
    }

    /// BigInt shift left: a n big-shl -> a << n
    pub(crate) fn builtin_big_shl(&mut self) -> Result<(), EvalError> {
        let n = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("big-shl requires shift amount on stack".to_string())
        })?;
        let shift = match &n {
            Value::Number(f) => *f as u64,
            Value::Literal(s) | Value::Output(s) => {
                s.trim().parse::<u64>().map_err(|_| {
                    EvalError::ExecError(format!("big-shl: invalid shift amount: {}", s))
                })?
            }
            _ => return Err(EvalError::ExecError("big-shl requires number shift amount".to_string())),
        };
        let a = self.pop_bigint("big-shl")?;
        self.stack.push(Value::BigInt(a << shift));
        self.last_exit_code = 0;
        Ok(())
    }

    /// BigInt shift right: a n big-shr -> a >> n
    pub(crate) fn builtin_big_shr(&mut self) -> Result<(), EvalError> {
        let n = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("big-shr requires shift amount on stack".to_string())
        })?;
        let shift = match &n {
            Value::Number(f) => *f as u64,
            Value::Literal(s) | Value::Output(s) => {
                s.trim().parse::<u64>().map_err(|_| {
                    EvalError::ExecError(format!("big-shr: invalid shift amount: {}", s))
                })?
            }
            _ => return Err(EvalError::ExecError("big-shr requires number shift amount".to_string())),
        };
        let a = self.pop_bigint("big-shr")?;
        self.stack.push(Value::BigInt(a >> shift));
        self.last_exit_code = 0;
        Ok(())
    }

    /// BigInt power: a n big-pow -> a^n
    pub(crate) fn builtin_big_pow(&mut self) -> Result<(), EvalError> {
        let n = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("big-pow requires exponent on stack".to_string())
        })?;
        let exp = match &n {
            Value::Number(f) => *f as u32,
            Value::Literal(s) | Value::Output(s) => {
                s.trim().parse::<u32>().map_err(|_| {
                    EvalError::ExecError(format!("big-pow: invalid exponent: {}", s))
                })?
            }
            _ => return Err(EvalError::ExecError("big-pow requires number exponent".to_string())),
        };
        let a = self.pop_bigint("big-pow")?;
        self.stack.push(Value::BigInt(a.pow(exp)));
        self.last_exit_code = 0;
        Ok(())
    }
}
