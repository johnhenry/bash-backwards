//! HTTP client operations for hsab
//!
//! Provides HTTP fetch operations using ureq (blocking):
//! - fetch: Make HTTP request, return body (auto-parse JSON)
//! - fetch-status: Return status code as number
//! - fetch-headers: Return response headers as Map

use super::{Evaluator, EvalError};
use crate::ast::Value;
use std::collections::HashMap;

impl Evaluator {
    /// fetch: [body] URL [method] fetch -> response
    /// Makes an HTTP request and returns the response body
    /// - If no method specified, defaults to GET
    /// - If response is JSON, parses it to Map
    /// - Otherwise returns body as string
    pub(crate) fn builtin_fetch(&mut self) -> Result<(), EvalError> {
        // Pop arguments in LIFO order
        // Stack could be: URL fetch
        //               : URL method fetch
        //               : body URL method fetch
        //               : headers body URL method fetch

        // First, let's peek at the stack to determine what we have
        let mut method = "GET".to_string();
        let url: String;
        let mut body: Option<String> = None;
        let mut headers: Option<HashMap<String, String>> = None;

        // Pop values from stack (reverse order)
        let mut args: Vec<Value> = Vec::new();
        while let Some(value) = self.stack.last() {
            match value {
                Value::Block(_) | Value::Marker => break,
                _ => {
                    args.push(self.stack.pop().unwrap());
                }
            }
            // Limit to avoid consuming too much
            if args.len() >= 4 {
                break;
            }
        }

        // Parse arguments based on count
        // args are in reverse order (last popped = first arg)
        args.reverse();

        match args.len() {
            0 => {
                return Err(EvalError::StackUnderflow("fetch requires URL".into()));
            }
            1 => {
                // Just URL
                url = args[0].as_arg().ok_or_else(|| {
                    EvalError::TypeError {
                        expected: "URL string".into(),
                        got: format!("{:?}", args[0]),
                    }
                })?;
            }
            2 => {
                // URL method or body URL
                // If first looks like a method, it's URL+method
                // Otherwise it's body+URL (method defaults to POST if body present)
                let first = args[0].as_arg().unwrap_or_default();
                let second = args[1].as_arg().unwrap_or_default();

                if is_http_method(&second) {
                    url = first;
                    method = second.to_uppercase();
                } else if is_url(&second) {
                    body = Some(first);
                    url = second;
                    method = "POST".to_string(); // Default to POST if body provided
                } else {
                    url = first;
                    method = second.to_uppercase();
                }
            }
            3 => {
                // body URL method
                body = Some(args[0].as_arg().unwrap_or_default());
                url = args[1].as_arg().unwrap_or_default();
                method = args[2].as_arg().unwrap_or_default().to_uppercase();
            }
            _ => {
                // headers body URL method (4 args)
                if let Value::Map(m) = &args[0] {
                    let mut h = HashMap::new();
                    for (k, v) in m {
                        if let Some(val) = v.as_arg() {
                            h.insert(k.clone(), val);
                        }
                    }
                    headers = Some(h);
                }
                body = Some(args[1].as_arg().unwrap_or_default());
                url = args[2].as_arg().unwrap_or_default();
                method = args[3].as_arg().unwrap_or_default().to_uppercase();
            }
        }

        // Make the request
        let response = self.do_http_request(&method, &url, body.as_deref(), headers.as_ref())?;

        // Auto-parse JSON if content-type indicates it
        let content_type = response.content_type.unwrap_or_default();
        let result = if content_type.contains("application/json") {
            // Try to parse as JSON
            match serde_json::from_str::<serde_json::Value>(&response.body) {
                Ok(json) => json_to_value(json),
                Err(_) => Value::Output(response.body),
            }
        } else {
            Value::Output(response.body)
        };

        self.stack.push(result);
        self.last_exit_code = if response.status >= 400 { 1 } else { 0 };
        Ok(())
    }

    /// fetch-status: URL [method] fetch-status -> status_code
    /// Makes an HTTP request and returns just the status code
    pub(crate) fn builtin_fetch_status(&mut self) -> Result<(), EvalError> {
        // Pop URL and optional method
        let mut method = "GET".to_string();
        let url_val = self.stack.pop().ok_or_else(|| {
            EvalError::StackUnderflow("fetch-status requires URL".into())
        })?;

        // Check if there's a method on top
        let url = if is_http_method(&url_val.as_arg().unwrap_or_default()) {
            method = url_val.as_arg().unwrap_or_default().to_uppercase();
            self.stack.pop()
                .ok_or_else(|| EvalError::StackUnderflow("fetch-status requires URL".into()))?
                .as_arg()
                .ok_or_else(|| EvalError::TypeError {
                    expected: "URL string".into(),
                    got: "non-string".into(),
                })?
        } else {
            url_val.as_arg().ok_or_else(|| EvalError::TypeError {
                expected: "URL string".into(),
                got: format!("{:?}", url_val),
            })?
        };

        let response = self.do_http_request(&method, &url, None, None)?;
        self.stack.push(Value::Number(response.status as f64));
        self.last_exit_code = if response.status >= 400 { 1 } else { 0 };
        Ok(())
    }

    /// fetch-headers: URL [method] fetch-headers -> headers_map
    /// Makes an HTTP request and returns response headers as a Map
    pub(crate) fn builtin_fetch_headers(&mut self) -> Result<(), EvalError> {
        // Pop URL and optional method
        let mut method = "GET".to_string();
        let url_val = self.stack.pop().ok_or_else(|| {
            EvalError::StackUnderflow("fetch-headers requires URL".into())
        })?;

        let url = if is_http_method(&url_val.as_arg().unwrap_or_default()) {
            method = url_val.as_arg().unwrap_or_default().to_uppercase();
            self.stack.pop()
                .ok_or_else(|| EvalError::StackUnderflow("fetch-headers requires URL".into()))?
                .as_arg()
                .ok_or_else(|| EvalError::TypeError {
                    expected: "URL string".into(),
                    got: "non-string".into(),
                })?
        } else {
            url_val.as_arg().ok_or_else(|| EvalError::TypeError {
                expected: "URL string".into(),
                got: format!("{:?}", url_val),
            })?
        };

        let response = self.do_http_request(&method, &url, None, None)?;

        // Convert headers to Map
        let headers_map: HashMap<String, Value> = response.headers
            .into_iter()
            .map(|(k, v)| (k, Value::Literal(v)))
            .collect();

        self.stack.push(Value::Map(headers_map));
        self.last_exit_code = if response.status >= 400 { 1 } else { 0 };
        Ok(())
    }

    /// Internal helper to make HTTP requests using ureq
    fn do_http_request(
        &self,
        method: &str,
        url: &str,
        body: Option<&str>,
        headers: Option<&HashMap<String, String>>,
    ) -> Result<HttpResponse, EvalError> {
        // Create request based on method
        let request = match method {
            "GET" => ureq::get(url),
            "POST" => ureq::post(url),
            "PUT" => ureq::put(url),
            "DELETE" => ureq::delete(url),
            "PATCH" => ureq::patch(url),
            "HEAD" => ureq::head(url),
            "OPTIONS" => ureq::request("OPTIONS", url),
            other => ureq::request(other, url),
        };

        // Add headers if provided
        let request = if let Some(h) = headers {
            let mut r = request;
            for (k, v) in h {
                r = r.set(k, v);
            }
            r
        } else {
            request
        };

        // Execute request (with or without body)
        let response = if let Some(b) = body {
            request
                .set("Content-Type", "application/json")
                .send_string(b)
        } else {
            request.call()
        };

        match response {
            Ok(resp) => {
                let status = resp.status();
                let content_type = Some(resp.content_type().to_string());

                // Collect headers
                let mut headers_map = HashMap::new();
                for name in resp.headers_names() {
                    if let Some(value) = resp.header(&name) {
                        headers_map.insert(name, value.to_string());
                    }
                }

                // Read body
                let body = resp.into_string().unwrap_or_default();

                Ok(HttpResponse {
                    status,
                    content_type,
                    headers: headers_map,
                    body,
                })
            }
            Err(ureq::Error::Status(code, resp)) => {
                // HTTP error (4xx/5xx)
                let content_type = Some(resp.content_type().to_string());
                let mut headers_map = HashMap::new();
                for name in resp.headers_names() {
                    if let Some(value) = resp.header(&name) {
                        headers_map.insert(name, value.to_string());
                    }
                }
                let body = resp.into_string().unwrap_or_default();

                Ok(HttpResponse {
                    status: code,
                    content_type,
                    headers: headers_map,
                    body,
                })
            }
            Err(e) => {
                Err(EvalError::ExecError(format!("HTTP request failed: {}", e)))
            }
        }
    }
}

/// Response from an HTTP request
struct HttpResponse {
    status: u16,
    content_type: Option<String>,
    headers: HashMap<String, String>,
    body: String,
}

/// Check if a string looks like an HTTP method
fn is_http_method(s: &str) -> bool {
    matches!(
        s.to_uppercase().as_str(),
        "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HEAD" | "OPTIONS"
    )
}

/// Check if a string looks like a URL
fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

/// Convert serde_json::Value to hsab Value
fn json_to_value(json: serde_json::Value) -> Value {
    match json {
        serde_json::Value::Null => Value::Nil,
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                Value::Number(f)
            } else {
                Value::Literal(n.to_string())
            }
        }
        serde_json::Value::String(s) => Value::Literal(s),
        serde_json::Value::Array(arr) => {
            Value::List(arr.into_iter().map(json_to_value).collect())
        }
        serde_json::Value::Object(obj) => {
            let map: HashMap<String, Value> = obj
                .into_iter()
                .map(|(k, v)| (k, json_to_value(v)))
                .collect();
            Value::Map(map)
        }
    }
}
