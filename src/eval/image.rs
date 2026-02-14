use super::{Evaluator, EvalError};
use crate::ast::Value;

impl Evaluator {
    /// Load an image file (stack version): "path/to/image.png" image-load -> Media value
    pub(crate) fn builtin_image_load_stack(&mut self) -> Result<(), EvalError> {
        let path_val = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("image-load requires a file path on stack".to_string())
        })?;

        let path = path_val.as_arg().ok_or_else(|| {
            EvalError::ExecError("image-load requires a string path".to_string())
        })?;

        self.load_image_from_path(&path)
    }

    /// Common image loading logic
    pub(crate) fn load_image_from_path(&mut self, path: &str) -> Result<(), EvalError> {

        // Expand tilde if present
        let expanded_path = if path.starts_with('~') {
            if let Ok(home) = std::env::var("HOME") {
                path.replacen('~', &home, 1)
            } else {
                path.to_string()
            }
        } else {
            path.to_string()
        };

        // Read the file
        let data = std::fs::read(&expanded_path).map_err(|e| {
            EvalError::ExecError(format!("Failed to read image file '{}': {}", path, e))
        })?;

        // Detect MIME type from extension
        let mime_type = match expanded_path.rsplit('.').next().map(|s| s.to_lowercase()).as_deref() {
            Some("png") => "image/png",
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("gif") => "image/gif",
            Some("webp") => "image/webp",
            Some("svg") => "image/svg+xml",
            Some("bmp") => "image/bmp",
            Some("ico") => "image/x-icon",
            Some("tiff") | Some("tif") => "image/tiff",
            _ => "application/octet-stream",
        };

        // Try to detect dimensions from PNG header
        let (width, height) = if mime_type == "image/png" && data.len() > 24 {
            // PNG: width at bytes 16-19, height at bytes 20-23 (big endian)
            let w = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
            let h = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
            (Some(w), Some(h))
        } else if mime_type == "image/jpeg" && data.len() > 2 {
            // JPEG: need to parse markers to find dimensions (complex)
            // For now, return None
            (None, None)
        } else if mime_type == "image/gif" && data.len() > 10 {
            // GIF: width at bytes 6-7, height at bytes 8-9 (little endian)
            let w = u16::from_le_bytes([data[6], data[7]]) as u32;
            let h = u16::from_le_bytes([data[8], data[9]]) as u32;
            (Some(w), Some(h))
        } else {
            (None, None)
        };

        self.stack.push(Value::Media {
            mime_type: mime_type.to_string(),
            data,
            width,
            height,
            alt: None,
            source: Some(path.to_string()),
        });

        self.last_exit_code = 0;
        Ok(())
    }

    /// Display a Media value: media image-show
    pub(crate) fn builtin_image_show(&mut self) -> Result<(), EvalError> {
        let media = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("image-show requires a Media value on stack".to_string())
        })?;

        match &media {
            Value::Media { .. } => {
                use crate::display::format_value;
                let output = format_value(&media, 80);
                println!("{}", output);
                // Push media back (non-destructive)
                self.stack.push(media);
                self.last_exit_code = 0;
            }
            _ => {
                self.stack.push(media);
                return Err(EvalError::ExecError("image-show requires a Media value".to_string()));
            }
        }

        Ok(())
    }

    /// Get info about a Media value: media image-info -> record
    pub(crate) fn builtin_image_info(&mut self) -> Result<(), EvalError> {
        let media = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("image-info requires a Media value on stack".to_string())
        })?;

        match media {
            Value::Media { mime_type, data, width, height, alt, source } => {
                let mut map = std::collections::HashMap::new();
                map.insert("mime_type".to_string(), Value::Literal(mime_type.clone()));
                map.insert("size".to_string(), Value::Number(data.len() as f64));
                if let Some(w) = width {
                    map.insert("width".to_string(), Value::Number(w as f64));
                }
                if let Some(h) = height {
                    map.insert("height".to_string(), Value::Number(h as f64));
                }
                if let Some(a) = &alt {
                    map.insert("alt".to_string(), Value::Literal(a.clone()));
                }
                if let Some(s) = &source {
                    map.insert("source".to_string(), Value::Literal(s.clone()));
                }
                self.stack.push(Value::Map(map));
                self.last_exit_code = 0;
            }
            other => {
                self.stack.push(other);
                return Err(EvalError::ExecError("image-info requires a Media value".to_string()));
            }
        }

        Ok(())
    }
}
