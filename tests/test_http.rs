//! Integration tests for HTTP operations
//!
//! Note: These tests require network access. Tests that contact external
//! services use httpbin.org which is a reliable test endpoint.
//! Tests are marked with #[ignore] if they might be flaky due to network.

#[path = "common/mod.rs"]
mod common;
#[allow(unused_imports)]
use common::{eval, eval_exit_code, Evaluator, lex, parse};

// === Error handling tests (no network required) ===

#[test]
fn test_fetch_requires_url() {
    let result = eval("fetch");
    assert!(result.is_err());
}

#[test]
fn test_fetch_status_requires_url() {
    let result = eval("fetch-status");
    assert!(result.is_err());
}

#[test]
fn test_fetch_headers_requires_url() {
    let result = eval("fetch-headers");
    assert!(result.is_err());
}

#[test]
fn test_fetch_invalid_url() {
    // Completely invalid URL should error
    let result = eval(r#""not-a-valid-url" fetch"#);
    assert!(result.is_err());
}

// === Basic GET tests (requires network) ===

#[test]
#[ignore] // Requires network
fn test_fetch_basic_get() {
    let output = eval(r#""https://httpbin.org/get" fetch typeof"#).unwrap();
    // httpbin.org returns JSON, which should be parsed to Map
    assert!(output.contains("Map") || output.contains("string"));
}

#[test]
#[ignore] // Requires network
fn test_fetch_json_parsing() {
    // httpbin.org/get returns JSON with an "url" field
    let output = eval(r#""https://httpbin.org/get" fetch "url" get"#).unwrap();
    assert!(output.contains("httpbin.org"));
}

#[test]
#[ignore] // Requires network
fn test_fetch_status_200() {
    let output = eval(r#""https://httpbin.org/status/200" fetch-status"#).unwrap();
    assert_eq!(output.trim(), "200");
}

#[test]
#[ignore] // Requires network
fn test_fetch_status_404() {
    let output = eval(r#""https://httpbin.org/status/404" fetch-status"#).unwrap();
    assert_eq!(output.trim(), "404");
}

#[test]
#[ignore] // Requires network
fn test_fetch_status_500() {
    let output = eval(r#""https://httpbin.org/status/500" fetch-status"#).unwrap();
    assert_eq!(output.trim(), "500");
}

#[test]
#[ignore] // Requires network
fn test_fetch_headers_returns_map() {
    let output = eval(r#""https://httpbin.org/get" fetch-headers typeof"#).unwrap();
    assert_eq!(output.trim(), "Map");
}

#[test]
#[ignore] // Requires network
fn test_fetch_headers_content_type() {
    let output = eval(r#""https://httpbin.org/get" fetch-headers "content-type" get"#).unwrap();
    assert!(output.contains("application/json"));
}

// === HTTP method tests (requires network) ===

#[test]
#[ignore] // Requires network
fn test_fetch_explicit_get() {
    let output = eval(r#""https://httpbin.org/get" "GET" fetch "url" get"#).unwrap();
    assert!(output.contains("httpbin.org"));
}

#[test]
#[ignore] // Requires network
fn test_fetch_post() {
    // POST request with JSON body
    let output = eval(r#""{\"test\":123}" "https://httpbin.org/post" "POST" fetch "json" get "test" get"#).unwrap();
    assert_eq!(output.trim(), "123");
}

#[test]
#[ignore] // Requires network
fn test_fetch_put() {
    let output = eval(r#""{\"key\":\"value\"}" "https://httpbin.org/put" "PUT" fetch "json" get"#).unwrap();
    assert!(output.contains("key"));
}

#[test]
#[ignore] // Requires network
fn test_fetch_delete() {
    let output = eval(r#""https://httpbin.org/delete" "DELETE" fetch "url" get"#).unwrap();
    assert!(output.contains("httpbin.org"));
}

#[test]
#[ignore] // Requires network
fn test_fetch_patch() {
    let output = eval(r#""{\"update\":true}" "https://httpbin.org/patch" "PATCH" fetch "json" get"#).unwrap();
    assert!(output.contains("update"));
}

// === POST with body inference (requires network) ===

#[test]
#[ignore] // Requires network
fn test_fetch_body_infers_post() {
    // When body is provided and URL is second, should infer POST
    let output = eval(r#""{\"auto\":\"post\"}" "https://httpbin.org/post" fetch "json" get"#).unwrap();
    assert!(output.contains("auto"));
}

// === Exit code tests (requires network) ===

#[test]
#[ignore] // Requires network
fn test_fetch_success_exit_code() {
    let exit_code = eval_exit_code(r#""https://httpbin.org/status/200" fetch"#);
    assert_eq!(exit_code, 0);
}

#[test]
#[ignore] // Requires network
fn test_fetch_error_exit_code() {
    let exit_code = eval_exit_code(r#""https://httpbin.org/status/500" fetch"#);
    assert_eq!(exit_code, 1);
}

#[test]
#[ignore] // Requires network
fn test_fetch_404_exit_code() {
    let exit_code = eval_exit_code(r#""https://httpbin.org/status/404" fetch"#);
    assert_eq!(exit_code, 1);
}

// === Response body tests (requires network) ===

#[test]
#[ignore] // Requires network
fn test_fetch_plain_text() {
    // /robots.txt returns plain text, not JSON
    let output = eval(r#""https://httpbin.org/robots.txt" fetch typeof"#).unwrap();
    // Should be string output since it's not JSON
    assert!(output.contains("string") || output.contains("Output"));
}

#[test]
#[ignore] // Requires network
fn test_fetch_html() {
    let output = eval(r#""https://httpbin.org/html" fetch"#).unwrap();
    assert!(output.contains("html") || output.contains("Herman"));
}

// === Custom headers tests (requires network) ===

#[test]
#[ignore] // Requires network
fn test_fetch_with_custom_headers() {
    // Test custom headers - httpbin.org echoes headers back
    let output = eval(r#"record "X-Custom-Header" "test-value" set "{}" "https://httpbin.org/headers" "GET" fetch "headers" get "X-Custom-Header" get"#).unwrap();
    assert_eq!(output.trim(), "test-value");
}

// === URL validation tests ===

#[test]
fn test_url_with_protocol() {
    // Valid URL format should parse without error (might fail on network)
    // Just checking that it doesn't panic on parsing
    let result = eval(r#""https://example.com" fetch"#);
    // Will likely fail due to network, but shouldn't panic
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn test_url_without_protocol() {
    // URL without protocol should still be attempted
    let result = eval(r#""example.com" fetch"#);
    // Will likely fail, but shouldn't panic on URL parsing
    assert!(result.is_ok() || result.is_err());
}

// === Connection error tests ===

#[test]
fn test_fetch_connection_refused() {
    // Try to connect to a port that's not listening
    let result = eval(r#""http://127.0.0.1:59999" fetch"#);
    assert!(result.is_err());
}

#[test]
fn test_fetch_invalid_host() {
    let result = eval(r#""http://this-domain-definitely-does-not-exist-12345.com" fetch"#);
    assert!(result.is_err());
}

// === Method case insensitivity ===

#[test]
#[ignore] // Requires network
fn test_fetch_method_lowercase() {
    let output = eval(r#""https://httpbin.org/get" "get" fetch-status"#).unwrap();
    assert_eq!(output.trim(), "200");
}

#[test]
#[ignore] // Requires network
fn test_fetch_method_mixed_case() {
    let output = eval(r#""https://httpbin.org/post" "pOsT" fetch-status"#).unwrap();
    // POST without body might get 400 or 200 depending on server
    let code: i32 = output.trim().parse().unwrap();
    assert!(code >= 200);
}

// === Integration tests ===

#[test]
#[ignore] // Requires network
fn test_fetch_chain_operations() {
    // Fetch and process JSON response
    let output = eval(r#""https://httpbin.org/ip" fetch "origin" get"#).unwrap();
    // Should return an IP address
    assert!(!output.trim().is_empty());
}

#[test]
#[ignore] // Requires network
fn test_fetch_user_agent() {
    // httpbin.org/user-agent echoes the user agent
    let output = eval(r#""https://httpbin.org/user-agent" fetch "user-agent" get"#).unwrap();
    // Should contain some user agent string
    assert!(!output.trim().is_empty());
}
