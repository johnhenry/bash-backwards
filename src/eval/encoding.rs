use super::{Evaluator, EvalError};
use crate::ast::Value;

impl Evaluator {
    /// Convert Bytes/Media/string to base64 string: data to-base64 -> "base64..."
    pub(crate) fn builtin_to_base64(&mut self) -> Result<(), EvalError> {
        use base64::{Engine as _, engine::general_purpose::STANDARD};

        let value = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("to-base64 requires a value on stack".to_string())
        })?;

        match value {
            Value::Bytes(data) => {
                let b64 = STANDARD.encode(&data);
                self.stack.push(Value::Literal(b64));
                self.last_exit_code = 0;
            }
            Value::Media { data, .. } => {
                let b64 = STANDARD.encode(&data);
                self.stack.push(Value::Literal(b64));
                self.last_exit_code = 0;
            }
            Value::Literal(s) => {
                let b64 = STANDARD.encode(s.as_bytes());
                self.stack.push(Value::Literal(b64));
                self.last_exit_code = 0;
            }
            Value::Output(s) => {
                let b64 = STANDARD.encode(s.as_bytes());
                self.stack.push(Value::Literal(b64));
                self.last_exit_code = 0;
            }
            other => {
                self.stack.push(other);
                return Err(EvalError::ExecError("to-base64 requires Bytes, Media, or string".to_string()));
            }
        }

        Ok(())
    }

    /// Convert base64 string to Bytes: "base64..." from-base64 -> Bytes
    pub(crate) fn builtin_from_base64(&mut self) -> Result<(), EvalError> {
        use base64::{Engine as _, engine::general_purpose::STANDARD};

        let b64_str = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("from-base64 requires base64 string on stack".to_string())
        })?;

        let b64 = b64_str.as_arg().ok_or_else(|| {
            EvalError::ExecError("from-base64 requires base64 string".to_string())
        })?;

        let data = STANDARD.decode(&b64).map_err(|e| {
            EvalError::ExecError(format!("Invalid base64: {}", e))
        })?;

        self.stack.push(Value::Bytes(data));
        self.last_exit_code = 0;
        Ok(())
    }

    /// Convert Bytes/BigInt to hex string: bytes to-hex -> "abcd..."
    pub(crate) fn builtin_to_hex(&mut self) -> Result<(), EvalError> {
        let value = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("to-hex requires a value on stack".to_string())
        })?;

        match value {
            Value::Bytes(data) => {
                let hex_str = hex::encode(&data);
                self.stack.push(Value::Literal(hex_str));
                self.last_exit_code = 0;
            }
            Value::BigInt(n) => {
                // Format as hex without leading zeros (unless 0)
                let hex_str = format!("{:x}", n);
                self.stack.push(Value::Literal(hex_str));
                self.last_exit_code = 0;
            }
            Value::Literal(s) => {
                let hex_str = hex::encode(s.as_bytes());
                self.stack.push(Value::Literal(hex_str));
                self.last_exit_code = 0;
            }
            Value::Output(s) => {
                let hex_str = hex::encode(s.as_bytes());
                self.stack.push(Value::Literal(hex_str));
                self.last_exit_code = 0;
            }
            other => {
                self.stack.push(other);
                return Err(EvalError::ExecError("to-hex requires Bytes, BigInt, or string".to_string()));
            }
        }

        Ok(())
    }

    /// Convert hex string to Bytes: "abcd..." from-hex -> Bytes
    pub(crate) fn builtin_from_hex(&mut self) -> Result<(), EvalError> {
        let hex_str = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("from-hex requires hex string on stack".to_string())
        })?;

        let hex = hex_str.as_arg().ok_or_else(|| {
            EvalError::ExecError("from-hex requires hex string".to_string())
        })?;

        let data = hex::decode(&hex).map_err(|e| {
            EvalError::ExecError(format!("Invalid hex: {}", e))
        })?;

        self.stack.push(Value::Bytes(data));
        self.last_exit_code = 0;
        Ok(())
    }

    /// Convert string to Bytes: "hello" as-bytes -> Bytes
    pub(crate) fn builtin_as_bytes(&mut self) -> Result<(), EvalError> {
        let value = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("as-bytes requires a value on stack".to_string())
        })?;

        match value {
            Value::Literal(s) => {
                self.stack.push(Value::Bytes(s.into_bytes()));
                self.last_exit_code = 0;
            }
            Value::Output(s) => {
                self.stack.push(Value::Bytes(s.into_bytes()));
                self.last_exit_code = 0;
            }
            Value::Bytes(data) => {
                // Already bytes, just return
                self.stack.push(Value::Bytes(data));
                self.last_exit_code = 0;
            }
            other => {
                self.stack.push(other);
                return Err(EvalError::ExecError("as-bytes requires string".to_string()));
            }
        }

        Ok(())
    }

    /// Convert Bytes/BigInt to list of numbers: bytes to-bytes -> [44, 242, ...]
    /// For BigInt, returns Bytes (not a list) - the raw byte representation
    pub(crate) fn builtin_to_bytes_list(&mut self) -> Result<(), EvalError> {
        let value = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("to-bytes requires Bytes or BigInt on stack".to_string())
        })?;

        match value {
            Value::Bytes(data) => {
                let list: Vec<Value> = data.iter()
                    .map(|&b| Value::Number(b as f64))
                    .collect();
                self.stack.push(Value::List(list));
                self.last_exit_code = 0;
            }
            Value::BigInt(n) => {
                // Convert BigInt to Bytes (big-endian)
                let data = n.to_bytes_be();
                self.stack.push(Value::Bytes(data));
                self.last_exit_code = 0;
            }
            other => {
                self.stack.push(other);
                return Err(EvalError::ExecError("to-bytes requires Bytes or BigInt".to_string()));
            }
        }

        Ok(())
    }

    /// Convert Bytes to UTF-8 string: bytes to-string -> "hello"
    pub(crate) fn builtin_bytes_to_string(&mut self) -> Result<(), EvalError> {
        let value = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("to-string requires Bytes on stack".to_string())
        })?;

        match value {
            Value::Bytes(data) => {
                let s = String::from_utf8(data).map_err(|e| {
                    EvalError::ExecError(format!("Invalid UTF-8: {}", e))
                })?;
                self.stack.push(Value::Literal(s));
                self.last_exit_code = 0;
            }
            other => {
                // Not bytes, just stringify with as_arg
                if let Some(s) = other.as_arg() {
                    self.stack.push(Value::Literal(s));
                    self.last_exit_code = 0;
                } else {
                    self.stack.push(other);
                    return Err(EvalError::ExecError("to-string requires convertible value".to_string()));
                }
            }
        }

        Ok(())
    }

    /// Get length of Bytes: bytes len -> number
    pub(crate) fn builtin_bytes_len(&mut self) -> Result<(), EvalError> {
        let value = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("len requires value on stack".to_string())
        })?;

        match value {
            Value::Bytes(data) => {
                self.stack.push(Value::Number(data.len() as f64));
                self.last_exit_code = 0;
                Ok(())
            }
            other => {
                // Put it back and let regular len handle it
                self.stack.push(other);
                Err(EvalError::ExecError("Not bytes - fallback to string len".to_string()))
            }
        }
    }

    // ========================================
    // Hash functions (SHA-2 and SHA-3)
    // ========================================

    /// SHA-256 hash: "hello" sha256 -> Bytes
    pub(crate) fn builtin_sha256(&mut self) -> Result<(), EvalError> {
        use sha2::{Sha256, Digest};

        let value = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("sha256 requires a value on stack".to_string())
        })?;

        let data = match &value {
            Value::Literal(s) => s.as_bytes().to_vec(),
            Value::Output(s) => s.as_bytes().to_vec(),
            Value::Bytes(b) => b.clone(),
            _ => {
                self.stack.push(value);
                return Err(EvalError::ExecError("sha256 requires string or Bytes".to_string()));
            }
        };

        let mut hasher = Sha256::new();
        hasher.update(&data);
        let hash = hasher.finalize();

        self.stack.push(Value::Bytes(hash.to_vec()));
        self.last_exit_code = 0;
        Ok(())
    }

    /// SHA-384 hash: "hello" sha384 -> Bytes
    pub(crate) fn builtin_sha384(&mut self) -> Result<(), EvalError> {
        use sha2::{Sha384, Digest};

        let value = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("sha384 requires a value on stack".to_string())
        })?;

        let data = match &value {
            Value::Literal(s) => s.as_bytes().to_vec(),
            Value::Output(s) => s.as_bytes().to_vec(),
            Value::Bytes(b) => b.clone(),
            _ => {
                self.stack.push(value);
                return Err(EvalError::ExecError("sha384 requires string or Bytes".to_string()));
            }
        };

        let mut hasher = Sha384::new();
        hasher.update(&data);
        let hash = hasher.finalize();

        self.stack.push(Value::Bytes(hash.to_vec()));
        self.last_exit_code = 0;
        Ok(())
    }

    /// SHA-512 hash: "hello" sha512 -> Bytes
    pub(crate) fn builtin_sha512(&mut self) -> Result<(), EvalError> {
        use sha2::{Sha512, Digest};

        let value = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("sha512 requires a value on stack".to_string())
        })?;

        let data = match &value {
            Value::Literal(s) => s.as_bytes().to_vec(),
            Value::Output(s) => s.as_bytes().to_vec(),
            Value::Bytes(b) => b.clone(),
            _ => {
                self.stack.push(value);
                return Err(EvalError::ExecError("sha512 requires string or Bytes".to_string()));
            }
        };

        let mut hasher = Sha512::new();
        hasher.update(&data);
        let hash = hasher.finalize();

        self.stack.push(Value::Bytes(hash.to_vec()));
        self.last_exit_code = 0;
        Ok(())
    }

    /// SHA3-256 hash: "hello" sha3-256 -> Bytes
    pub(crate) fn builtin_sha3_256(&mut self) -> Result<(), EvalError> {
        use sha3::{Sha3_256, Digest};

        let value = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("sha3-256 requires a value on stack".to_string())
        })?;

        let data = match &value {
            Value::Literal(s) => s.as_bytes().to_vec(),
            Value::Output(s) => s.as_bytes().to_vec(),
            Value::Bytes(b) => b.clone(),
            _ => {
                self.stack.push(value);
                return Err(EvalError::ExecError("sha3-256 requires string or Bytes".to_string()));
            }
        };

        let mut hasher = Sha3_256::new();
        hasher.update(&data);
        let hash = hasher.finalize();

        self.stack.push(Value::Bytes(hash.to_vec()));
        self.last_exit_code = 0;
        Ok(())
    }

    /// SHA3-384 hash: "hello" sha3-384 -> Bytes
    pub(crate) fn builtin_sha3_384(&mut self) -> Result<(), EvalError> {
        use sha3::{Sha3_384, Digest};

        let value = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("sha3-384 requires a value on stack".to_string())
        })?;

        let data = match &value {
            Value::Literal(s) => s.as_bytes().to_vec(),
            Value::Output(s) => s.as_bytes().to_vec(),
            Value::Bytes(b) => b.clone(),
            _ => {
                self.stack.push(value);
                return Err(EvalError::ExecError("sha3-384 requires string or Bytes".to_string()));
            }
        };

        let mut hasher = Sha3_384::new();
        hasher.update(&data);
        let hash = hasher.finalize();

        self.stack.push(Value::Bytes(hash.to_vec()));
        self.last_exit_code = 0;
        Ok(())
    }

    /// SHA3-512 hash: "hello" sha3-512 -> Bytes
    pub(crate) fn builtin_sha3_512(&mut self) -> Result<(), EvalError> {
        use sha3::{Sha3_512, Digest};

        let value = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("sha3-512 requires a value on stack".to_string())
        })?;

        let data = match &value {
            Value::Literal(s) => s.as_bytes().to_vec(),
            Value::Output(s) => s.as_bytes().to_vec(),
            Value::Bytes(b) => b.clone(),
            _ => {
                self.stack.push(value);
                return Err(EvalError::ExecError("sha3-512 requires string or Bytes".to_string()));
            }
        };

        let mut hasher = Sha3_512::new();
        hasher.update(&data);
        let hash = hasher.finalize();

        self.stack.push(Value::Bytes(hash.to_vec()));
        self.last_exit_code = 0;
        Ok(())
    }

    /// SHA-256 hash of file: "path" sha256-file -> Bytes
    pub(crate) fn builtin_sha256_file(&mut self) -> Result<(), EvalError> {
        use sha2::{Sha256, Digest};
        use std::io::Read;

        let value = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("sha256-file requires path on stack".to_string())
        })?;

        let path = value.as_arg().ok_or_else(|| {
            EvalError::ExecError("sha256-file requires path string".to_string())
        })?;

        let mut file = std::fs::File::open(&path).map_err(|e| {
            EvalError::ExecError(format!("sha256-file: {}: {}", path, e))
        })?;

        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];
        loop {
            let n = file.read(&mut buffer).map_err(|e| {
                EvalError::ExecError(format!("sha256-file read error: {}", e))
            })?;
            if n == 0 { break; }
            hasher.update(&buffer[..n]);
        }

        let hash = hasher.finalize();
        self.stack.push(Value::Bytes(hash.to_vec()));
        self.last_exit_code = 0;
        Ok(())
    }

    /// Read N bytes from a file: "path" N read-bytes -> Bytes
    /// Useful for reading binary data or random bytes from /dev/urandom
    pub(crate) fn builtin_read_bytes(&mut self) -> Result<(), EvalError> {
        use std::io::Read;

        let n = self.pop_number("read-bytes")? as usize;
        let path = self.pop_string()?;

        let mut file = std::fs::File::open(&path).map_err(|e| {
            EvalError::ExecError(format!("read-bytes: {}: {}", path, e))
        })?;

        let mut buf = vec![0u8; n];
        file.read_exact(&mut buf).map_err(|e| {
            EvalError::ExecError(format!("read-bytes: {}: {}", path, e))
        })?;

        self.stack.push(Value::Bytes(buf));
        self.last_exit_code = 0;
        Ok(())
    }

    /// SHA3-256 hash of file: "path" sha3-256-file -> Bytes
    pub(crate) fn builtin_sha3_256_file(&mut self) -> Result<(), EvalError> {
        use sha3::{Sha3_256, Digest};
        use std::io::Read;

        let value = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("sha3-256-file requires path on stack".to_string())
        })?;

        let path = value.as_arg().ok_or_else(|| {
            EvalError::ExecError("sha3-256-file requires path string".to_string())
        })?;

        let mut file = std::fs::File::open(&path).map_err(|e| {
            EvalError::ExecError(format!("sha3-256-file: {}: {}", path, e))
        })?;

        let mut hasher = Sha3_256::new();
        let mut buffer = [0u8; 8192];
        loop {
            let n = file.read(&mut buffer).map_err(|e| {
                EvalError::ExecError(format!("sha3-256-file read error: {}", e))
            })?;
            if n == 0 { break; }
            hasher.update(&buffer[..n]);
        }

        let hash = hasher.finalize();
        self.stack.push(Value::Bytes(hash.to_vec()));
        self.last_exit_code = 0;
        Ok(())
    }
}
