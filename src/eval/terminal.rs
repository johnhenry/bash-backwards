use super::{Evaluator, EvalError};
use crate::ast::Value;

impl Evaluator {
    /// Create a Link value: url link -> Link  OR  url text link -> Link
    /// The link will be displayed as a clickable hyperlink in supported terminals
    pub(crate) fn builtin_link(&mut self) -> Result<(), EvalError> {
        // Check if we have 2 items (url + text) or 1 item (url only)
        let top = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("link requires URL on stack".to_string())
        })?;

        let url = top.as_arg().ok_or_else(|| {
            EvalError::ExecError("link requires URL string".to_string())
        })?;

        // Check if there's another item that could be text
        let text = if let Some(prev) = self.stack.last() {
            if let Some(text_str) = prev.as_arg() {
                // Pop the text and use URL from top
                self.stack.pop();
                Some(text_str)
            } else {
                None
            }
        } else {
            None
        };

        // If we found text, the order was: text url link
        // If not, it was just: url link
        let (final_url, final_text) = if text.is_some() {
            // text was first, url was second
            (url, text)
        } else {
            (url, None)
        };

        self.stack.push(Value::Link {
            url: final_url,
            text: final_text,
        });

        self.last_exit_code = 0;
        Ok(())
    }

    /// Get link info as a record: Link link-info -> {url: "...", text: "..."}
    pub(crate) fn builtin_link_info(&mut self) -> Result<(), EvalError> {
        let link = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError("link-info requires a Link value on stack".to_string())
        })?;

        match link {
            Value::Link { url, text } => {
                let mut map = std::collections::HashMap::new();
                map.insert("url".to_string(), Value::Literal(url));
                if let Some(t) = text {
                    map.insert("text".to_string(), Value::Literal(t));
                }
                self.stack.push(Value::Map(map));
                self.last_exit_code = 0;
            }
            other => {
                self.stack.push(other);
                return Err(EvalError::ExecError("link-info requires a Link value".to_string()));
            }
        }

        Ok(())
    }

    /// Copy value to system clipboard using OSC 52
    /// value .copy -> (value unchanged, data copied to clipboard)
    pub(crate) fn builtin_clip_copy(&mut self) -> Result<(), EvalError> {
        use base64::{Engine as _, engine::general_purpose::STANDARD};

        let value = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError(".copy requires a value on stack".to_string())
        })?;

        // Get string representation of value
        let text = value.as_arg().ok_or_else(|| {
            self.stack.push(value.clone());
            EvalError::ExecError(".copy requires a value with string representation".to_string())
        })?;

        // Encode as base64 for OSC 52
        let b64 = STANDARD.encode(text.as_bytes());

        // Send OSC 52 sequence to copy to clipboard
        // Format: ESC ] 52 ; c ; <base64-data> BEL
        // 'c' means the clipboard selection (as opposed to primary selection)
        print!("\x1b]52;c;{}\x07", b64);
        std::io::Write::flush(&mut std::io::stdout()).ok();

        // Push value back (non-destructive)
        self.stack.push(value);
        self.last_exit_code = 0;
        Ok(())
    }

    /// Copy value to clipboard and drop it from stack (destructive)
    /// value .cut -> ()
    pub(crate) fn builtin_clip_cut(&mut self) -> Result<(), EvalError> {
        use base64::{Engine as _, engine::general_purpose::STANDARD};

        let value = self.stack.pop().ok_or_else(|| {
            EvalError::ExecError(".cut requires a value on stack".to_string())
        })?;

        let text = value.as_arg().ok_or_else(|| {
            EvalError::ExecError(".cut requires a value with string representation".to_string())
        })?;

        let b64 = STANDARD.encode(text.as_bytes());
        print!("\x1b]52;c;{}\x07", b64);
        std::io::Write::flush(&mut std::io::stdout()).ok();

        // Don't push value back (destructive)
        self.last_exit_code = 0;
        Ok(())
    }

    /// Query the system clipboard using OSC 52 and return contents
    #[cfg(unix)]
    pub(crate) fn query_clipboard(&self) -> Result<String, EvalError> {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        use std::io::{Read, Write};
        use std::os::unix::io::AsRawFd;

        let stdin_fd = std::io::stdin().as_raw_fd();

        // Check if stdin is a TTY
        if unsafe { libc::isatty(stdin_fd) } == 0 {
            return Err(EvalError::ExecError("Clipboard access requires a terminal".to_string()));
        }

        // Save current terminal settings
        let mut orig_termios: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(stdin_fd, &mut orig_termios) } != 0 {
            return Err(EvalError::ExecError("Failed to get terminal attributes".to_string()));
        }

        // Set raw mode (disable canonical mode and echo)
        let mut raw = orig_termios;
        raw.c_lflag &= !(libc::ICANON | libc::ECHO);
        if unsafe { libc::tcsetattr(stdin_fd, libc::TCSANOW, &raw) } != 0 {
            return Err(EvalError::ExecError("Failed to set raw mode".to_string()));
        }

        // Helper to restore terminal
        let restore = |fd: i32, termios: &libc::termios| {
            unsafe { libc::tcsetattr(fd, libc::TCSANOW, termios) };
        };

        // Send OSC 52 query: request clipboard contents
        print!("\x1b]52;c;?\x07");
        if std::io::stdout().flush().is_err() {
            restore(stdin_fd, &orig_termios);
            return Err(EvalError::ExecError("Failed to send clipboard query".to_string()));
        }

        // Read response with timeout
        // Response format: ESC ] 52 ; c ; <base64> BEL  (or ESC \)
        let mut response = Vec::new();
        let mut stdin = std::io::stdin();
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(500);

        // Set non-blocking read using poll
        let mut poll_fd = [libc::pollfd {
            fd: stdin_fd,
            events: libc::POLLIN,
            revents: 0,
        }];

        loop {
            if start.elapsed() > timeout {
                restore(stdin_fd, &orig_termios);
                return Err(EvalError::ExecError(
                    "Clipboard query timed out (terminal may not support OSC 52)".to_string()
                ));
            }

            // Poll for input with short timeout
            let poll_result = unsafe {
                libc::poll(poll_fd.as_mut_ptr(), 1, 50)
            };

            if poll_result > 0 && (poll_fd[0].revents & libc::POLLIN) != 0 {
                let mut buf = [0u8; 1];
                if stdin.read(&mut buf).unwrap_or(0) == 1 {
                    response.push(buf[0]);

                    // Check for end of response (BEL or ESC \)
                    if buf[0] == 0x07 {
                        break;
                    }
                    if response.len() >= 2 {
                        let len = response.len();
                        if response[len-2] == 0x1b && response[len-1] == b'\\' {
                            break;
                        }
                    }
                }
            }
        }

        // Restore terminal
        restore(stdin_fd, &orig_termios);

        // Parse response: ESC ] 52 ; c ; <base64> BEL
        let response_str = String::from_utf8_lossy(&response);

        // Find the base64 data between "52;c;" and the terminator
        let start_marker = "52;c;";
        let start_pos = response_str.find(start_marker)
            .ok_or_else(|| EvalError::ExecError("Invalid clipboard response".to_string()))?;

        let b64_start = start_pos + start_marker.len();
        let b64_data: String = response_str[b64_start..]
            .chars()
            .take_while(|&c| c != '\x07' && c != '\x1b')
            .collect();

        if b64_data.is_empty() || b64_data == "?" {
            // Empty clipboard or query echo (not supported)
            return Ok(String::new());
        }

        // Decode base64
        let decoded = STANDARD.decode(&b64_data).map_err(|e| {
            EvalError::ExecError(format!("Failed to decode clipboard data: {}", e))
        })?;

        String::from_utf8(decoded).map_err(|e| {
            EvalError::ExecError(format!("Clipboard data is not valid UTF-8: {}", e))
        })
    }

    #[cfg(not(unix))]
    pub(crate) fn query_clipboard(&self) -> Result<String, EvalError> {
        Err(EvalError::ExecError("Clipboard access is only supported on Unix".to_string()))
    }

    /// Paste from system clipboard using OSC 52 query
    /// .paste -> value
    pub(crate) fn builtin_clip_paste(&mut self) -> Result<(), EvalError> {
        let text = self.query_clipboard()?;
        self.stack.push(Value::Literal(text));
        self.last_exit_code = 0;
        Ok(())
    }
}
