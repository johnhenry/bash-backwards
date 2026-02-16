//! Integration tests for encoding operations

#[path = "common/mod.rs"]
mod common;
#[allow(unused_imports)]
use common::{eval, eval_exit_code, Evaluator, lex, parse};

#[test]
fn test_hash_builtin_no_args() {
    // .hash with no args should show cache (initially empty)
    let exit_code = eval_exit_code(".hash");
    assert_eq!(exit_code, 0);
}

#[test]
fn test_hash_specific_command() {
    // .hash a command to add it to cache
    let exit_code = eval_exit_code("ls .hash");
    assert_eq!(exit_code, 0);
}

#[test]
fn test_hash_r_clears_cache() {
    // .hash -r should clear the cache
    let exit_code = eval_exit_code("-r .hash");
    assert_eq!(exit_code, 0);
}

#[test]
fn test_sha256_returns_bytes() {
    // sha256 should return Bytes type
    let output = eval(r#""hello" sha256 typeof"#).unwrap();
    assert_eq!(output.trim(), "bytes");
}

#[test]
fn test_sha256_to_hex() {
    let output = eval(r#""hello" sha256 to-hex"#).unwrap();
    // SHA-256 of "hello" is known
    assert_eq!(output.trim(), "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
}

#[test]
fn test_sha384_to_hex() {
    let output = eval(r#""hello" sha384 to-hex"#).unwrap();
    // SHA-384 of "hello"
    assert_eq!(output.trim(), "59e1748777448c69de6b800d7a33bbfb9ff1b463e44354c3553bcdb9c666fa90125a3c79f90397bdf5f6a13de828684f");
}

#[test]
fn test_sha512_to_hex() {
    let output = eval(r#""hello" sha512 to-hex"#).unwrap();
    // SHA-512 of "hello"
    assert_eq!(output.trim(), "9b71d224bd62f3785d96d46ad3ea3d73319bfbc2890caadae2dff72519673ca72323c3d99ba5c11d7c7acc6e14b8c5da0c4663475c2e5c3adef46f73bcdec043");
}

#[test]
fn test_sha3_256_to_hex() {
    let output = eval(r#""hello" sha3-256 to-hex"#).unwrap();
    // SHA3-256 of "hello"
    assert_eq!(output.trim(), "3338be694f50c5f338814986cdf0686453a888b84f424d792af4b9202398f392");
}

#[test]
fn test_sha3_384_to_hex() {
    let output = eval(r#""hello" sha3-384 to-hex"#).unwrap();
    // SHA3-384 of "hello"
    assert_eq!(output.trim(), "720aea11019ef06440fbf05d87aa24680a2153df3907b23631e7177ce620fa1330ff07c0fddee54699a4c3ee0ee9d887");
}

#[test]
fn test_sha3_512_to_hex() {
    let output = eval(r#""hello" sha3-512 to-hex"#).unwrap();
    // SHA3-512 of "hello"
    assert_eq!(output.trim(), "75d527c368f2efe848ecf6b073a36767800805e9eef2b1857d5f984f036eb6df891d75f72d9b154518c1cd58835286d1da9a38deba3de98b5a53e5ed78a84976");
}

#[test]
fn test_sha256_to_base64() {
    let output = eval(r#""hello" sha256 to-base64"#).unwrap();
    // Base64 of SHA-256 of "hello"
    assert_eq!(output.trim(), "LPJNul+wow4m6DsqxbninhsWHlwfp0JecwQzYpOLmCQ=");
}

#[test]
fn test_sha256_to_bytes_list() {
    let output = eval(r#""hello" sha256 to-bytes"#).unwrap();
    // Should be a list starting with [44, 242, 77, ...
    assert!(output.contains("44") && output.contains("242"));
}

#[test]
fn test_sha256_file() {
    use std::fs;
    let temp = tempfile::NamedTempFile::new().unwrap();
    fs::write(temp.path(), "hello").unwrap();
    
    let input = format!(r#""{}" sha256-file to-hex"#, temp.path().display());
    let output = eval(&input).unwrap();
    // Same as sha256 of "hello"
    assert_eq!(output.trim(), "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
}

#[test]
fn test_sha3_256_file() {
    use std::fs;
    let temp = tempfile::NamedTempFile::new().unwrap();
    fs::write(temp.path(), "hello").unwrap();
    
    let input = format!(r#""{}" sha3-256-file to-hex"#, temp.path().display());
    let output = eval(&input).unwrap();
    assert_eq!(output.trim(), "3338be694f50c5f338814986cdf0686453a888b84f424d792af4b9202398f392");
}

#[test]
fn test_bytes_equality() {
    let exit_code = eval_exit_code(r#""hello" sha256 "hello" sha256 eq?"#);
    assert_eq!(exit_code, 0, "Same hash should be equal");
}

#[test]
fn test_bytes_inequality() {
    let exit_code = eval_exit_code(r#""hello" sha256 "world" sha256 eq?"#);
    assert_eq!(exit_code, 1, "Different hashes should not be equal");
}

#[test]
fn test_as_bytes_string() {
    let output = eval(r#""hello" as-bytes to-hex"#).unwrap();
    // "hello" as hex bytes
    assert_eq!(output.trim(), "68656c6c6f");
}

#[test]
fn test_from_hex() {
    let output = eval(r#""68656c6c6f" from-hex to-string"#).unwrap();
    assert_eq!(output.trim(), "hello");
}

#[test]
fn test_from_base64() {
    let output = eval(r#""aGVsbG8=" from-base64 to-string"#).unwrap();
    assert_eq!(output.trim(), "hello");
}

#[test]
fn test_hex_roundtrip() {
    let output = eval(r#""hello" as-bytes to-hex from-hex to-string"#).unwrap();
    assert_eq!(output.trim(), "hello");
}

#[test]
fn test_base64_roundtrip() {
    let output = eval(r#""hello" as-bytes to-base64 from-base64 to-string"#).unwrap();
    assert_eq!(output.trim(), "hello");
}

#[test]
fn test_typeof_bytes() {
    let output = eval(r#""hello" sha256 typeof"#).unwrap();
    assert_eq!(output.trim(), "bytes");
}

#[test]
fn test_bytes_len() {
    let output = eval(r#""hello" sha256 len"#).unwrap();
    assert_eq!(output.trim(), "32"); // SHA-256 is 32 bytes
}

#[test]
fn test_sha512_len() {
    let output = eval(r#""hello" sha512 len"#).unwrap();
    assert_eq!(output.trim(), "64"); // SHA-512 is 64 bytes
}

#[test]
fn test_cross_encoding_hex_to_base64() {
    let output = eval(r#""68656c6c6f" from-hex to-base64"#).unwrap();
    assert_eq!(output.trim(), "aGVsbG8=");
}

#[test]
fn test_empty_string_sha256() {
    let output = eval(r#""" sha256 to-hex"#).unwrap();
    // SHA-256 of empty string
    assert_eq!(output.trim(), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}


// === Recovered tests ===

#[test]
fn test_sort_nums_single() {
    let output = eval(r#"'[42]' into-json sort-nums to-json"#).unwrap();
    assert_eq!(output.trim(), "[42.0]");
}

#[test]
fn test_empty_string_to_hex() {
    let output = eval(r#""" as-bytes to-hex"#).unwrap();
    assert_eq!(output.trim(), "");
}

#[test]
fn test_empty_string_to_base64() {
    let output = eval(r#""" to-base64"#).unwrap();
    assert_eq!(output.trim(), "");
}

#[test]
fn test_empty_hex_from_hex() {
    let output = eval(r#""" from-hex to-hex"#).unwrap();
    assert_eq!(output.trim(), "");
}

#[test]
fn test_empty_base64_from_base64() {
    let output = eval(r#""" from-base64 to-hex"#).unwrap();
    assert_eq!(output.trim(), "");
}

#[test]
fn test_empty_string_sha384() {
    let output = eval(r#""" sha384 to-hex"#).unwrap();
    // SHA-384 of empty string (known value)
    assert_eq!(output.trim(), "38b060a751ac96384cd9327eb1b1e36a21fdb71114be07434c0cc7bf63f6e1da274edebfe76f65fbd51ad2f14898b95b");
}

#[test]
fn test_empty_string_sha512() {
    let output = eval(r#""" sha512 to-hex"#).unwrap();
    // SHA-512 of empty string (known value)
    assert_eq!(output.trim(), "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e");
}

#[test]
fn test_empty_string_sha3_256() {
    let output = eval(r#""" sha3-256 to-hex"#).unwrap();
    // SHA3-256 of empty string (known value)
    assert_eq!(output.trim(), "a7ffc6f8bf1ed76651c14756a061d662f580ff4de43b49fa82d80a4b80f8434a");
}

#[test]
fn test_empty_string_sha3_384() {
    let output = eval(r#""" sha3-384 to-hex"#).unwrap();
    // SHA3-384 of empty string (known value)
    assert_eq!(output.trim(), "0c63a75b845e4f7d01107d852e4c2485c51a50aaaa94fc61995e71bbee983a2ac3713831264adb47fb6bd1e058d5f004");
}

#[test]
fn test_empty_string_sha3_512() {
    let output = eval(r#""" sha3-512 to-hex"#).unwrap();
    // SHA3-512 of empty string (known value)
    assert_eq!(output.trim(), "a69f73cca23a9ac5c8b567dc185a756e97c982164fe25859e0d1dcc1475c80a615b2123af1f5f94c11e3e9402c3ac558f500199d95b6d3e301758586281dcd26");
}

#[test]
fn test_empty_bytes_len() {
    let output = eval(r#""" as-bytes len"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_sha384_len() {
    let output = eval(r#""hello" sha384 len"#).unwrap();
    assert_eq!(output.trim(), "48"); // SHA-384 is 48 bytes
}

#[test]
fn test_sha3_256_len() {
    let output = eval(r#""hello" sha3-256 len"#).unwrap();
    assert_eq!(output.trim(), "32"); // SHA3-256 is 32 bytes
}

#[test]
fn test_sha3_384_len() {
    let output = eval(r#""hello" sha3-384 len"#).unwrap();
    assert_eq!(output.trim(), "48"); // SHA3-384 is 48 bytes
}

#[test]
fn test_sha3_512_len() {
    let output = eval(r#""hello" sha3-512 len"#).unwrap();
    assert_eq!(output.trim(), "64"); // SHA3-512 is 64 bytes
}

#[test]
fn test_binary_data_hex_roundtrip() {
    // Use hex with non-UTF8 bytes (0xff, 0x00, etc.)
    let output = eval(r#""ff00fe01" from-hex to-hex"#).unwrap();
    assert_eq!(output.trim(), "ff00fe01");
}

#[test]
fn test_binary_data_base64_roundtrip() {
    // Non-UTF8 binary data via hex, then base64 roundtrip
    let output = eval(r#""ff00fe01" from-hex to-base64 from-base64 to-hex"#).unwrap();
    assert_eq!(output.trim(), "ff00fe01");
}

#[test]
fn test_binary_null_bytes() {
    // Data with null bytes
    let output = eval(r#""00000000" from-hex to-hex"#).unwrap();
    assert_eq!(output.trim(), "00000000");
}

#[test]
fn test_binary_all_ff() {
    // Data with all 0xFF bytes
    let output = eval(r#""ffffffff" from-hex to-hex"#).unwrap();
    assert_eq!(output.trim(), "ffffffff");
}

#[test]
fn test_binary_data_sha256() {
    // Hash binary data (not valid UTF-8)
    let output = eval(r#""ff00fe01" from-hex sha256 to-hex"#).unwrap();
    // SHA-256 should work on binary data
    assert_eq!(output.trim().len(), 64); // 32 bytes = 64 hex chars
}

#[test]
fn test_binary_data_sha3_256() {
    // Hash binary data with SHA3
    let output = eval(r#""ff00fe01" from-hex sha3-256 to-hex"#).unwrap();
    assert_eq!(output.trim().len(), 64); // 32 bytes = 64 hex chars
}

#[test]
fn test_hex_base64_hex_roundtrip() {
    let output = eval(r#""deadbeef" from-hex to-base64 from-base64 to-hex"#).unwrap();
    assert_eq!(output.trim(), "deadbeef");
}

#[test]
fn test_base64_hex_base64_roundtrip() {
    let output = eval(r#""SGVsbG8gV29ybGQh" from-base64 to-hex from-hex to-base64"#).unwrap();
    assert_eq!(output.trim(), "SGVsbG8gV29ybGQh");
}

#[test]
fn test_string_bytes_string_roundtrip() {
    let output = eval(r#""Hello, World!" as-bytes to-string"#).unwrap();
    assert_eq!(output.trim(), "Hello, World!");
}

#[test]
fn test_hash_hex_base64_consistency() {
    // sha256 -> to-hex -> from-hex -> to-base64 should equal sha256 -> to-base64
    let hex_path = eval(r#""test" sha256 to-hex from-hex to-base64"#).unwrap();
    let direct_path = eval(r#""test" sha256 to-base64"#).unwrap();
    assert_eq!(hex_path.trim(), direct_path.trim());
}

#[test]
fn test_as_bytes_on_bytes_is_idempotent() {
    // as-bytes on already-bytes should pass through
    let output = eval(r#""hello" as-bytes as-bytes to-hex"#).unwrap();
    assert_eq!(output.trim(), "68656c6c6f");
}

#[test]
fn test_as_bytes_from_hex_data() {
    // Bytes from hex through as-bytes
    let output = eval(r#""cafebabe" from-hex as-bytes to-hex"#).unwrap();
    assert_eq!(output.trim(), "cafebabe");
}

#[test]
fn test_invalid_hex_error() {
    let result = eval(r#""not_hex_zz" from-hex"#);
    assert!(result.is_err(), "Invalid hex should produce error");
}

#[test]
fn test_odd_length_hex_error() {
    let result = eval(r#""abc" from-hex"#);
    assert!(result.is_err(), "Odd-length hex should produce error");
}

#[test]
fn test_invalid_base64_error() {
    let result = eval(r#""not_valid_base64!!!" from-base64"#);
    assert!(result.is_err(), "Invalid base64 should produce error");
}

#[test]
fn test_invalid_utf8_to_string_error() {
    // Create bytes that are not valid UTF-8
    let result = eval(r#""ff" from-hex to-string"#);
    assert!(result.is_err(), "Invalid UTF-8 bytes should error on to-string");
}

#[test]
fn test_hex_uppercase_input() {
    let output = eval(r#""DEADBEEF" from-hex to-hex"#).unwrap();
    assert_eq!(output.trim(), "deadbeef"); // Output is lowercase
}

#[test]
fn test_hex_mixed_case_input() {
    let output = eval(r#""DeAdBeEf" from-hex to-hex"#).unwrap();
    assert_eq!(output.trim(), "deadbeef");
}

#[test]
fn test_long_string_sha256() {
    // Hash a long string (1000 'a' characters)
    let long_input = "a".repeat(1000);
    let input = format!(r#""{}" sha256 to-hex"#, long_input);
    let output = eval(&input).unwrap();
    // SHA-256 of 1000 'a's (known value)
    assert_eq!(output.trim(), "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3");
}

#[test]
fn test_long_hex_roundtrip() {
    // 256 bytes of data
    let hex_data = "ab".repeat(256);
    let input = format!(r#""{}" from-hex to-hex"#, hex_data);
    let output = eval(&input).unwrap();
    assert_eq!(output.trim(), hex_data);
}

#[test]
fn test_long_base64_roundtrip() {
    // Encode and decode a long string
    let input = "This is a test string that should be encoded to base64 and back again without any data loss whatsoever.";
    let cmd = format!(r#""{}" to-base64 from-base64 to-string"#, input);
    let output = eval(&cmd).unwrap();
    assert_eq!(output.trim(), input);
}

#[test]
fn test_sha256_empty_file() {
    use std::fs;
    let temp = tempfile::NamedTempFile::new().unwrap();
    fs::write(temp.path(), "").unwrap();

    let input = format!(r#""{}" sha256-file to-hex"#, temp.path().display());
    let output = eval(&input).unwrap();
    // SHA-256 of empty file (same as empty string)
    assert_eq!(output.trim(), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}

#[test]
fn test_sha3_256_empty_file() {
    use std::fs;
    let temp = tempfile::NamedTempFile::new().unwrap();
    fs::write(temp.path(), "").unwrap();

    let input = format!(r#""{}" sha3-256-file to-hex"#, temp.path().display());
    let output = eval(&input).unwrap();
    // SHA3-256 of empty file
    assert_eq!(output.trim(), "a7ffc6f8bf1ed76651c14756a061d662f580ff4de43b49fa82d80a4b80f8434a");
}

#[test]
fn test_sha256_binary_file() {
    use std::fs;
    let temp = tempfile::NamedTempFile::new().unwrap();
    // Write binary data (not valid UTF-8)
    fs::write(temp.path(), &[0xff, 0x00, 0xfe, 0x01]).unwrap();

    let input = format!(r#""{}" sha256-file to-hex"#, temp.path().display());
    let output = eval(&input).unwrap();
    // Should hash successfully
    assert_eq!(output.trim().len(), 64);
}

#[test]
fn test_sha256_file_nonexistent() {
    let result = eval(r#""/nonexistent/path/to/file" sha256-file"#);
    assert!(result.is_err(), "Non-existent file should produce error");
}

#[test]
fn test_sha3_256_file_nonexistent() {
    let result = eval(r#""/nonexistent/path/to/file" sha3-256-file"#);
    assert!(result.is_err(), "Non-existent file should produce error");
}

#[test]
fn test_to_bytes_list_values() {
    let output = eval(r#""hello" as-bytes to-bytes"#).unwrap();
    // "hello" = [104, 101, 108, 108, 111]
    assert!(output.contains("104"));
    assert!(output.contains("101"));
    assert!(output.contains("108"));
    assert!(output.contains("111"));
}

#[test]
fn test_to_bytes_list_binary() {
    let output = eval(r#""ff00" from-hex to-bytes"#).unwrap();
    // [255, 0]
    assert!(output.contains("255"));
    assert!(output.contains("0"));
}

#[test]
fn test_typeof_from_hex_result() {
    let output = eval(r#""deadbeef" from-hex typeof"#).unwrap();
    assert_eq!(output.trim(), "bytes");
}

#[test]
fn test_typeof_from_base64_result() {
    let output = eval(r#""aGVsbG8=" from-base64 typeof"#).unwrap();
    assert_eq!(output.trim(), "bytes");
}

#[test]
fn test_typeof_as_bytes_result() {
    let output = eval(r#""hello" as-bytes typeof"#).unwrap();
    assert_eq!(output.trim(), "bytes");
}

#[test]
fn test_sha384_equality() {
    let exit_code = eval_exit_code(r#""hello" sha384 "hello" sha384 eq?"#);
    assert_eq!(exit_code, 0, "Same SHA-384 hash should be equal");
}

#[test]
fn test_sha384_inequality() {
    let exit_code = eval_exit_code(r#""hello" sha384 "world" sha384 eq?"#);
    assert_eq!(exit_code, 1, "Different SHA-384 hashes should not be equal");
}

#[test]
fn test_sha512_equality() {
    let exit_code = eval_exit_code(r#""hello" sha512 "hello" sha512 eq?"#);
    assert_eq!(exit_code, 0, "Same SHA-512 hash should be equal");
}

#[test]
fn test_sha3_256_equality() {
    let exit_code = eval_exit_code(r#""hello" sha3-256 "hello" sha3-256 eq?"#);
    assert_eq!(exit_code, 0, "Same SHA3-256 hash should be equal");
}

#[test]
fn test_sha3_512_equality() {
    let exit_code = eval_exit_code(r#""hello" sha3-512 "hello" sha3-512 eq?"#);
    assert_eq!(exit_code, 0, "Same SHA3-512 hash should be equal");
}

#[test]
fn test_different_hash_algorithms_not_equal() {
    let exit_code = eval_exit_code(r#""hello" sha256 "hello" sha3-256 eq?"#);
    assert_eq!(exit_code, 1, "SHA-256 and SHA3-256 of same input should not be equal");
}

#[test]
fn test_cross_encoding_base64_to_hex() {
    // Base64 "aGVsbG8=" decodes to "hello", which is hex 68656c6c6f
    let output = eval(r#""aGVsbG8=" from-base64 to-hex"#).unwrap();
    assert_eq!(output.trim(), "68656c6c6f");
}

#[test]
fn test_to_base64_string() {
    let output = eval(r#""hello" to-base64"#).unwrap();
    assert_eq!(output.trim(), "aGVsbG8=");
}

#[test]
fn test_to_base64_bytes() {
    let output = eval(r#""hello" as-bytes to-base64"#).unwrap();
    assert_eq!(output.trim(), "aGVsbG8=");
}

#[test]
fn test_to_hex_string() {
    let output = eval(r#""hello" to-hex"#).unwrap();
    assert_eq!(output.trim(), "68656c6c6f");
}

#[test]
fn test_to_hex_bytes() {
    let output = eval(r#""hello" as-bytes to-hex"#).unwrap();
    assert_eq!(output.trim(), "68656c6c6f");
}

#[test]
fn test_encode_unicode() {
    // Unicode character encoding
    let output = eval(r#""\u{1f600}" to-base64 from-base64 to-string"#).unwrap();
    // This tests that unicode roundtrips correctly
    assert!(!output.is_empty());
}

#[test]
fn test_hash_unicode() {
    // Hash unicode string
    let output = eval(r#""cafe\u{0301}" sha256 to-hex"#).unwrap();
    assert_eq!(output.trim().len(), 64); // Valid 32-byte hash
}

#[test]
fn test_encode_newlines() {
    let output = eval(r#""line1\nline2" as-bytes to-hex"#).unwrap();
    // Should contain 0a (newline character)
    assert!(output.contains("0a"));
}

#[test]
fn test_encode_tabs() {
    let output = eval(r#""a\tb" as-bytes to-hex"#).unwrap();
    // Should contain 09 (tab character)
    assert!(output.contains("09"));
}

#[test]
fn test_base64_no_padding() {
    // "abc" encodes to "YWJj" (no padding)
    let output = eval(r#""abc" to-base64"#).unwrap();
    assert_eq!(output.trim(), "YWJj");
}

#[test]
fn test_base64_one_pad() {
    // "ab" encodes to "YWI=" (one padding)
    let output = eval(r#""ab" to-base64"#).unwrap();
    assert_eq!(output.trim(), "YWI=");
}

#[test]
fn test_base64_two_pad() {
    // "a" encodes to "YQ==" (two padding)
    let output = eval(r#""a" to-base64"#).unwrap();
    assert_eq!(output.trim(), "YQ==");
}

#[test]
fn test_bigint_zero_to_hex() {
    let output = eval(r#""0" to-bigint to-hex"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_bigint_one_to_hex() {
    let output = eval(r#""1" to-bigint to-hex"#).unwrap();
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_bigint_255_to_hex() {
    let output = eval(r#""255" to-bigint to-hex"#).unwrap();
    assert_eq!(output.trim(), "ff");
}

#[test]
fn test_bigint_256_to_hex() {
    let output = eval(r#""256" to-bigint to-hex"#).unwrap();
    assert_eq!(output.trim(), "100");
}

#[test]
fn test_bigint_large_to_hex() {
    // 2^64 = 18446744073709551616
    let output = eval(r#""18446744073709551616" to-bigint to-hex"#).unwrap();
    assert_eq!(output.trim(), "10000000000000000");
}

#[test]
fn test_bigint_zero_to_bytes() {
    let output = eval(r#""0" to-bigint to-bytes to-hex"#).unwrap();
    // BigInt 0 should produce empty or single zero byte
    assert!(output.trim() == "" || output.trim() == "00");
}

#[test]
fn test_bigint_small_to_bytes() {
    let output = eval(r#""255" to-bigint to-bytes to-hex"#).unwrap();
    assert_eq!(output.trim(), "ff");
}

#[test]
fn test_chained_hash_operations() {
    // Hash, then hash the hash
    let output = eval(r#""hello" sha256 sha256 to-hex"#).unwrap();
    // Double SHA-256 is commonly used (e.g., Bitcoin)
    assert_eq!(output.trim().len(), 64);
}

#[test]
fn test_chained_encode_decode() {
    // Multiple encode/decode cycles
    let output = eval(r#""test" as-bytes to-base64 from-base64 to-hex from-hex to-string"#).unwrap();
    assert_eq!(output.trim(), "test");
}

#[test]
fn test_hash_of_hash_deterministic() {
    let hash1 = eval(r#""hello" sha256 sha256 to-hex"#).unwrap();
    let hash2 = eval(r#""hello" sha256 sha256 to-hex"#).unwrap();
    assert_eq!(hash1.trim(), hash2.trim(), "Hash of hash should be deterministic");
}

#[test]
fn test_bigint_mod_smaller_than_divisor() {
    // When dividend < divisor, mod returns dividend
    let output = eval(r#""5" to-bigint "10" to-bigint big-mod to-string"#).unwrap();
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_to_hex_string_direct() {
    let output = eval(r#""hello" to-hex"#).unwrap();
    assert_eq!(output.trim(), "68656c6c6f");
}

#[test]
fn test_to_hex_bytes_direct() {
    let output = eval(r#""hello" as-bytes to-hex"#).unwrap();
    assert_eq!(output.trim(), "68656c6c6f");
}

#[test]
fn test_bigint_zero_to_hex_encoding() {
    let output = eval(r#""0" to-bigint to-hex"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_bigint_one_to_hex_encoding() {
    let output = eval(r#""1" to-bigint to-hex"#).unwrap();
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_bigint_255_to_hex_encoding() {
    let output = eval(r#""255" to-bigint to-hex"#).unwrap();
    assert_eq!(output.trim(), "ff");
}

#[test]
fn test_bigint_256_to_hex_encoding() {
    let output = eval(r#""256" to-bigint to-hex"#).unwrap();
    assert_eq!(output.trim(), "100");
}

#[test]
fn test_bigint_large_to_hex_encoding() {
    // 2^64 = 18446744073709551616
    let output = eval(r#""18446744073709551616" to-bigint to-hex"#).unwrap();
    assert_eq!(output.trim(), "10000000000000000");
}

#[test]
fn test_bigint_zero_to_bytes_encoding() {
    let output = eval(r#""0" to-bigint to-bytes to-hex"#).unwrap();
    // BigInt 0 should produce empty bytes (no leading zeros)
    assert!(output.trim() == "" || output.trim() == "00");
}

#[test]
fn test_bigint_small_to_bytes_encoding() {
    let output = eval(r#""255" to-bigint to-bytes to-hex"#).unwrap();
    assert_eq!(output.trim(), "ff");
}

#[test]
fn test_sha256_string_vs_bytes_same() {
    // SHA-256 of string should equal SHA-256 of equivalent bytes
    let from_string = eval(r#""hello" sha256 to-hex"#).unwrap();
    let from_bytes = eval(r#""68656c6c6f" from-hex sha256 to-hex"#).unwrap();
    assert_eq!(from_string.trim(), from_bytes.trim());
}

#[test]
fn test_sha3_256_string_vs_bytes_same() {
    let from_string = eval(r#""hello" sha3-256 to-hex"#).unwrap();
    let from_bytes = eval(r#""68656c6c6f" from-hex sha3-256 to-hex"#).unwrap();
    assert_eq!(from_string.trim(), from_bytes.trim());
}

#[test]
fn test_space_encoding() {
    let output = eval(r#"" " as-bytes to-hex"#).unwrap();
    assert_eq!(output.trim(), "20"); // Space is 0x20
}

#[test]
fn test_sha256_large_file() {
    use std::fs;
    let temp = tempfile::NamedTempFile::new().unwrap();
    // Write 10KB of data
    fs::write(temp.path(), vec![0xab; 10240]).unwrap();

    let input = format!(r#""{}" sha256-file to-hex"#, temp.path().display());
    let output = eval(&input).unwrap();
    // Should produce valid 64-char hex hash
    assert_eq!(output.trim().len(), 64);
}

#[test]
fn test_sha3_256_large_file() {
    use std::fs;
    let temp = tempfile::NamedTempFile::new().unwrap();
    // Write 10KB of data
    fs::write(temp.path(), vec![0xab; 10240]).unwrap();

    let input = format!(r#""{}" sha3-256-file to-hex"#, temp.path().display());
    let output = eval(&input).unwrap();
    // Should produce valid 64-char hex hash
    assert_eq!(output.trim().len(), 64);
}

#[test]
fn test_sha256_file_vs_string_consistency() {
    use std::fs;
    let temp = tempfile::NamedTempFile::new().unwrap();
    fs::write(temp.path(), "test data").unwrap();

    let file_hash = format!(r#""{}" sha256-file to-hex"#, temp.path().display());
    let file_output = eval(&file_hash).unwrap();
    let string_output = eval(r#""test data" sha256 to-hex"#).unwrap();
    assert_eq!(file_output.trim(), string_output.trim());
}

#[test]
fn test_sha3_256_file_vs_string_consistency() {
    use std::fs;
    let temp = tempfile::NamedTempFile::new().unwrap();
    fs::write(temp.path(), "test data").unwrap();

    let file_hash = format!(r#""{}" sha3-256-file to-hex"#, temp.path().display());
    let file_output = eval(&file_hash).unwrap();
    let string_output = eval(r#""test data" sha3-256 to-hex"#).unwrap();
    assert_eq!(file_output.trim(), string_output.trim());
}

#[test]
fn test_null_byte_via_hex() {
    // Verify null byte (0x00) survives roundtrip through hex encoding
    let output = eval(r#""00" from-hex to-hex"#).unwrap();
    assert_eq!(output.trim(), "00"); // Null byte is 0x00
}

// === read-bytes tests ===

#[test]
fn test_read_bytes_from_urandom() {
    // Read 32 random bytes, verify it produces Bytes type of correct length
    let output = eval(r#""/dev/urandom" 32 read-bytes len"#).unwrap();
    assert_eq!(output.trim(), "32");
}

#[test]
fn test_read_bytes_typeof() {
    let output = eval(r#""/dev/urandom" 16 read-bytes typeof"#).unwrap();
    assert_eq!(output.trim(), "bytes");
}

#[test]
fn test_read_bytes_to_hex() {
    // Read 4 bytes, convert to hex - should be 8 hex chars
    let output = eval(r#""/dev/urandom" 4 read-bytes to-hex"#).unwrap();
    assert_eq!(output.trim().len(), 8);
}

#[test]
fn test_read_bytes_to_bigint() {
    // The full pipeline: random bytes -> BigInt
    let output = eval(r#""/dev/urandom" 32 read-bytes to-bigint typeof"#).unwrap();
    assert_eq!(output.trim(), "bigint");
}

#[test]
fn test_read_bytes_from_regular_file() {
    use std::fs;
    let temp = tempfile::NamedTempFile::new().unwrap();
    fs::write(temp.path(), "Hello, World!").unwrap();

    // Read first 5 bytes: "Hello"
    let input = format!(r#""{}" 5 read-bytes to-string"#, temp.path().display());
    let output = eval(&input).unwrap();
    assert_eq!(output.trim(), "Hello");
}

#[test]
fn test_read_bytes_nonexistent_file() {
    let result = eval(r#""/nonexistent/file" 10 read-bytes"#);
    assert!(result.is_err());
}

#[test]
fn test_read_bytes_zero() {
    // Reading 0 bytes should work
    let output = eval(r#""/dev/urandom" 0 read-bytes len"#).unwrap();
    assert_eq!(output.trim(), "0");
}

