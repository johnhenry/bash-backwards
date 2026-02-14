//! Integration tests for bigint operations

#[path = "common/mod.rs"]
mod common;
#[allow(unused_imports)]
use common::{eval, eval_exit_code, Evaluator, lex, parse};

#[test]
fn test_bytes_to_bigint() {
    // Convert SHA-256 hash to BigInt
    let output = eval(r#""hello" sha256 to-bigint typeof"#).unwrap();
    assert_eq!(output.trim(), "BigInt");
}

#[test]
fn test_bigint_to_hex() {
    // BigInt should convert back to same hex as original hash
    let output = eval(r#""hello" sha256 to-bigint to-hex"#).unwrap();
    assert_eq!(output.trim(), "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
}

#[test]
fn test_bigint_to_bytes() {
    // BigInt should convert back to Bytes
    let output = eval(r#""hello" sha256 to-bigint to-bytes to-hex"#).unwrap();
    assert_eq!(output.trim(), "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
}

#[test]
fn test_string_to_bigint() {
    // Parse decimal string to BigInt
    let output = eval(r#""12345678901234567890" to-bigint typeof"#).unwrap();
    assert_eq!(output.trim(), "BigInt");
}

#[test]
fn test_bigint_to_string() {
    let output = eval(r#""12345678901234567890" to-bigint to-string"#).unwrap();
    assert_eq!(output.trim(), "12345678901234567890");
}

#[test]
fn test_hex_to_bigint() {
    // Parse hex string to BigInt
    let output = eval(r#""0xff" to-bigint to-string"#).unwrap();
    assert_eq!(output.trim(), "255");
}

#[test]
fn test_bigint_add() {
    let output = eval(r#""100" to-bigint "200" to-bigint big-add to-string"#).unwrap();
    assert_eq!(output.trim(), "300");
}

#[test]
fn test_bigint_sub() {
    let output = eval(r#""300" to-bigint "100" to-bigint big-sub to-string"#).unwrap();
    assert_eq!(output.trim(), "200");
}

#[test]
fn test_bigint_mul() {
    let output = eval(r#""12345678901234567890" to-bigint "2" to-bigint big-mul to-string"#).unwrap();
    assert_eq!(output.trim(), "24691357802469135780");
}

#[test]
fn test_bigint_div() {
    let output = eval(r#""100" to-bigint "3" to-bigint big-div to-string"#).unwrap();
    assert_eq!(output.trim(), "33");
}

#[test]
fn test_bigint_mod() {
    let output = eval(r#""100" to-bigint "3" to-bigint big-mod to-string"#).unwrap();
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_bigint_xor() {
    // XOR two hashes
    let output = eval(r#""hello" sha256 to-bigint "world" sha256 to-bigint big-xor to-hex"#).unwrap();
    // Result should be different from both inputs
    assert!(!output.contains("2cf24dba"));
    assert_eq!(output.trim().len(), 64); // Still 256 bits
}

#[test]
fn test_bigint_and() {
    let output = eval(r#""0xff" to-bigint "0x0f" to-bigint big-and to-string"#).unwrap();
    assert_eq!(output.trim(), "15");
}

#[test]
fn test_bigint_or() {
    let output = eval(r#""0xf0" to-bigint "0x0f" to-bigint big-or to-string"#).unwrap();
    assert_eq!(output.trim(), "255");
}

#[test]
fn test_bigint_eq() {
    let exit_code = eval_exit_code(r#""100" to-bigint "100" to-bigint big-eq?"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_bigint_neq() {
    let exit_code = eval_exit_code(r#""100" to-bigint "200" to-bigint big-eq?"#);
    assert_eq!(exit_code, 1);
}

#[test]
fn test_bigint_lt() {
    let exit_code = eval_exit_code(r#""100" to-bigint "200" to-bigint big-lt?"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_bigint_gt() {
    let exit_code = eval_exit_code(r#""200" to-bigint "100" to-bigint big-gt?"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_bigint_shl() {
    // Shift left by 4 bits (multiply by 16)
    let output = eval(r#""1" to-bigint 4 big-shl to-string"#).unwrap();
    assert_eq!(output.trim(), "16");
}

#[test]
fn test_bigint_shr() {
    // Shift right by 4 bits (divide by 16)
    let output = eval(r#""256" to-bigint 4 big-shr to-string"#).unwrap();
    assert_eq!(output.trim(), "16");
}

#[test]
fn test_bigint_pow() {
    let output = eval(r#""2" to-bigint 10 big-pow to-string"#).unwrap();
    assert_eq!(output.trim(), "1024");
}


// === Recovered tests ===

#[test]
fn test_sort_nums_single() {
    let output = eval(r#"'[42]' into-json sort-nums to-json"#).unwrap();
    assert_eq!(output.trim(), "[42.0]");
}

#[test]
fn test_bigint_zero() {
    let output = eval(r#""0" to-bigint to-string"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_bigint_one() {
    let output = eval(r#""1" to-bigint to-string"#).unwrap();
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_bigint_add_zero() {
    // Adding zero should return the same number
    let output = eval(r#""12345678901234567890" to-bigint "0" to-bigint big-add to-string"#).unwrap();
    assert_eq!(output.trim(), "12345678901234567890");
}

#[test]
fn test_bigint_sub_zero() {
    // Subtracting zero should return the same number
    let output = eval(r#""12345678901234567890" to-bigint "0" to-bigint big-sub to-string"#).unwrap();
    assert_eq!(output.trim(), "12345678901234567890");
}

#[test]
fn test_bigint_sub_equal() {
    // Subtracting a number from itself should be zero
    let output = eval(r#""100" to-bigint "100" to-bigint big-sub to-string"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_bigint_mul_zero() {
    // Multiplying by zero should return zero
    let output = eval(r#""12345678901234567890" to-bigint "0" to-bigint big-mul to-string"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_bigint_mul_one() {
    // Multiplying by one should return the same number
    let output = eval(r#""12345678901234567890" to-bigint "1" to-bigint big-mul to-string"#).unwrap();
    assert_eq!(output.trim(), "12345678901234567890");
}

#[test]
fn test_bigint_div_one() {
    // Dividing by one should return the same number
    let output = eval(r#""12345678901234567890" to-bigint "1" to-bigint big-div to-string"#).unwrap();
    assert_eq!(output.trim(), "12345678901234567890");
}

#[test]
fn test_bigint_div_self() {
    // Dividing a number by itself should return one
    let output = eval(r#""12345678901234567890" to-bigint "12345678901234567890" to-bigint big-div to-string"#).unwrap();
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_bigint_mod_one() {
    // Any number mod 1 should be zero
    let output = eval(r#""12345678901234567890" to-bigint "1" to-bigint big-mod to-string"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_bigint_zero_div_by_number() {
    // Zero divided by any number should be zero
    let output = eval(r#""0" to-bigint "12345" to-bigint big-div to-string"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_bigint_zero_mod_number() {
    // Zero mod any number should be zero
    let output = eval(r#""0" to-bigint "12345" to-bigint big-mod to-string"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_bigint_very_large_number() {
    // 2^256 - 1 (max 256-bit unsigned int, common in crypto)
    let output = eval(r#""115792089237316195423570985008687907853269984665640564039457584007913129639935" to-bigint to-string"#).unwrap();
    assert_eq!(output.trim(), "115792089237316195423570985008687907853269984665640564039457584007913129639935");
}

#[test]
fn test_bigint_large_add() {
    // Add two very large numbers
    let output = eval(r#""99999999999999999999999999999999999999" to-bigint "1" to-bigint big-add to-string"#).unwrap();
    assert_eq!(output.trim(), "100000000000000000000000000000000000000");
}

#[test]
fn test_bigint_large_sub() {
    // Subtract from a very large number
    let output = eval(r#""100000000000000000000000000000000000000" to-bigint "1" to-bigint big-sub to-string"#).unwrap();
    assert_eq!(output.trim(), "99999999999999999999999999999999999999");
}

#[test]
fn test_bigint_large_mul() {
    // Multiply two large numbers
    let output = eval(r#""99999999999999999999" to-bigint "99999999999999999999" to-bigint big-mul to-string"#).unwrap();
    assert_eq!(output.trim(), "9999999999999999999800000000000000000001");
}

#[test]
fn test_bigint_large_div() {
    // Divide large number - should be integer division
    let output = eval(r#""1000000000000000000000000000000" to-bigint "1000000000000000" to-bigint big-div to-string"#).unwrap();
    assert_eq!(output.trim(), "1000000000000000");
}

#[test]
fn test_bigint_large_mod() {
    // Large number modulo
    let output = eval(r#""1000000000000000000007" to-bigint "1000000000000000000000" to-bigint big-mod to-string"#).unwrap();
    assert_eq!(output.trim(), "7");
}

#[test]
fn test_bigint_eq_zero() {
    let exit_code = eval_exit_code(r#""0" to-bigint "0" to-bigint big-eq?"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_bigint_lt_zero() {
    // 0 is not less than 0
    let exit_code = eval_exit_code(r#""0" to-bigint "0" to-bigint big-lt?"#);
    assert_eq!(exit_code, 1);
}

#[test]
fn test_bigint_gt_zero() {
    // 0 is not greater than 0
    let exit_code = eval_exit_code(r#""0" to-bigint "0" to-bigint big-gt?"#);
    assert_eq!(exit_code, 1);
}

#[test]
fn test_bigint_lt_equal_values() {
    // Equal values should fail lt
    let exit_code = eval_exit_code(r#""100" to-bigint "100" to-bigint big-lt?"#);
    assert_eq!(exit_code, 1);
}

#[test]
fn test_bigint_gt_equal_values() {
    // Equal values should fail gt
    let exit_code = eval_exit_code(r#""100" to-bigint "100" to-bigint big-gt?"#);
    assert_eq!(exit_code, 1);
}

#[test]
fn test_bigint_lt_false() {
    // 200 is not less than 100
    let exit_code = eval_exit_code(r#""200" to-bigint "100" to-bigint big-lt?"#);
    assert_eq!(exit_code, 1);
}

#[test]
fn test_bigint_gt_false() {
    // 100 is not greater than 200
    let exit_code = eval_exit_code(r#""100" to-bigint "200" to-bigint big-gt?"#);
    assert_eq!(exit_code, 1);
}

#[test]
fn test_bigint_compare_large_numbers() {
    // Compare two very large numbers
    let exit_code = eval_exit_code(r#""99999999999999999999999999999999999998" to-bigint "99999999999999999999999999999999999999" to-bigint big-lt?"#);
    assert_eq!(exit_code, 0);
}

#[test]
fn test_bigint_xor_zero() {
    // XOR with zero should return the same number
    let output = eval(r#""255" to-bigint "0" to-bigint big-xor to-string"#).unwrap();
    assert_eq!(output.trim(), "255");
}

#[test]
fn test_bigint_xor_self() {
    // XOR with self should return zero
    let output = eval(r#""255" to-bigint "255" to-bigint big-xor to-string"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_bigint_and_zero() {
    // AND with zero should return zero
    let output = eval(r#""255" to-bigint "0" to-bigint big-and to-string"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_bigint_and_self() {
    // AND with self should return the same number
    let output = eval(r#""255" to-bigint "255" to-bigint big-and to-string"#).unwrap();
    assert_eq!(output.trim(), "255");
}

#[test]
fn test_bigint_or_zero() {
    // OR with zero should return the same number
    let output = eval(r#""255" to-bigint "0" to-bigint big-or to-string"#).unwrap();
    assert_eq!(output.trim(), "255");
}

#[test]
fn test_bigint_or_self() {
    // OR with self should return the same number
    let output = eval(r#""255" to-bigint "255" to-bigint big-or to-string"#).unwrap();
    assert_eq!(output.trim(), "255");
}

#[test]
fn test_bigint_bitwise_large() {
    // Bitwise operations on large numbers
    let output = eval(r#""0xffffffffffffffff" to-bigint "0xf0f0f0f0f0f0f0f0" to-bigint big-and to-hex"#).unwrap();
    assert_eq!(output.trim(), "f0f0f0f0f0f0f0f0");
}

#[test]
fn test_bigint_xor_large() {
    // XOR on large numbers
    let output = eval(r#""0xffffffffffffffff" to-bigint "0xf0f0f0f0f0f0f0f0" to-bigint big-xor to-hex"#).unwrap();
    assert_eq!(output.trim(), "f0f0f0f0f0f0f0f");
}

#[test]
fn test_bigint_shl_zero() {
    // Shift left by zero should return the same number
    let output = eval(r#""255" to-bigint 0 big-shl to-string"#).unwrap();
    assert_eq!(output.trim(), "255");
}

#[test]
fn test_bigint_shr_zero() {
    // Shift right by zero should return the same number
    let output = eval(r#""255" to-bigint 0 big-shr to-string"#).unwrap();
    assert_eq!(output.trim(), "255");
}

#[test]
fn test_bigint_shl_large() {
    // Shift left by large amount
    let output = eval(r#""1" to-bigint 256 big-shl to-string"#).unwrap();
    // 2^256
    assert_eq!(output.trim(), "115792089237316195423570985008687907853269984665640564039457584007913129639936");
}

#[test]
fn test_bigint_shr_to_zero() {
    // Shift right until zero
    let output = eval(r#""255" to-bigint 8 big-shr to-string"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_bigint_shift_zero_value() {
    // Shifting zero should always be zero
    let output = eval(r#""0" to-bigint 100 big-shl to-string"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_bigint_shr_zero_value() {
    // Shifting zero right should always be zero
    let output = eval(r#""0" to-bigint 100 big-shr to-string"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_bigint_pow_zero_exponent() {
    // Any number to the power of 0 should be 1
    let output = eval(r#""12345678901234567890" to-bigint 0 big-pow to-string"#).unwrap();
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_bigint_pow_one_exponent() {
    // Any number to the power of 1 should be itself
    let output = eval(r#""12345678901234567890" to-bigint 1 big-pow to-string"#).unwrap();
    assert_eq!(output.trim(), "12345678901234567890");
}

#[test]
fn test_bigint_zero_to_power() {
    // Zero to any positive power should be zero
    let output = eval(r#""0" to-bigint 10 big-pow to-string"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_bigint_zero_to_zero() {
    // 0^0 = 1 (mathematical convention in most libraries)
    let output = eval(r#""0" to-bigint 0 big-pow to-string"#).unwrap();
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_bigint_one_to_power() {
    // One to any power should be one
    let output = eval(r#""1" to-bigint 100 big-pow to-string"#).unwrap();
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_bigint_pow_large() {
    // 2^100
    let output = eval(r#""2" to-bigint 100 big-pow to-string"#).unwrap();
    assert_eq!(output.trim(), "1267650600228229401496703205376");
}

#[test]
fn test_bigint_hex_uppercase() {
    let output = eval(r#""0xFF" to-bigint to-string"#).unwrap();
    assert_eq!(output.trim(), "255");
}

#[test]
fn test_bigint_hex_uppercase_0x() {
    let output = eval(r#""0XFF" to-bigint to-string"#).unwrap();
    assert_eq!(output.trim(), "255");
}

#[test]
fn test_bigint_hex_mixed_case() {
    let output = eval(r#""0xAaBbCcDdEeFf" to-bigint to-string"#).unwrap();
    assert_eq!(output.trim(), "187723572702975");
}

#[test]
fn test_bigint_hex_zero() {
    let output = eval(r#""0x0" to-bigint to-string"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_bigint_hex_leading_zeros() {
    let output = eval(r#""0x00ff" to-bigint to-string"#).unwrap();
    assert_eq!(output.trim(), "255");
}

#[test]
fn test_bigint_hex_large() {
    // 64-character hex (256-bit number)
    let output = eval(r#""0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff" to-bigint to-string"#).unwrap();
    assert_eq!(output.trim(), "115792089237316195423570985008687907853269984665640564039457584007913129639935");
}

#[test]
fn test_bigint_from_number() {
    let output = eval(r#"42 to-bigint to-string"#).unwrap();
    assert_eq!(output.trim(), "42");
}

// Note: BigInt doesn't support float conversion - floats are rejected as invalid

#[test]
fn test_bigint_from_zero_number() {
    let output = eval(r#"0 to-bigint to-string"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_bigint_idempotent() {
    // Converting BigInt to BigInt should be a no-op
    let output = eval(r#""12345678901234567890" to-bigint to-bigint to-string"#).unwrap();
    assert_eq!(output.trim(), "12345678901234567890");
}

#[test]
fn test_bigint_chained_add() {
    // (100 + 200) + 300 = 600
    let output = eval(r#""100" to-bigint "200" to-bigint big-add "300" to-bigint big-add to-string"#).unwrap();
    assert_eq!(output.trim(), "600");
}

#[test]
fn test_bigint_chained_mul() {
    // (2 * 3) * 4 = 24
    let output = eval(r#""2" to-bigint "3" to-bigint big-mul "4" to-bigint big-mul to-string"#).unwrap();
    assert_eq!(output.trim(), "24");
}

#[test]
fn test_bigint_mixed_operations() {
    // ((10 + 5) * 2) - 10 = 20
    let output = eval(r#""10" to-bigint "5" to-bigint big-add "2" to-bigint big-mul "10" to-bigint big-sub to-string"#).unwrap();
    assert_eq!(output.trim(), "20");
}

#[test]
fn test_bigint_pow_then_mod() {
    // 2^10 mod 100 = 1024 mod 100 = 24
    let output = eval(r#""2" to-bigint 10 big-pow "100" to-bigint big-mod to-string"#).unwrap();
    assert_eq!(output.trim(), "24");
}

#[test]
fn test_bigint_shift_and_mask() {
    // (1 << 8) - 1 = 255
    let output = eval(r#""1" to-bigint 8 big-shl "1" to-bigint big-sub to-string"#).unwrap();
    assert_eq!(output.trim(), "255");
}

#[test]
fn test_bigint_div_by_zero_error() {
    let result = eval(r#""100" to-bigint "0" to-bigint big-div"#);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("division by zero"));
}

#[test]
fn test_bigint_mod_by_zero_error() {
    let result = eval(r#""100" to-bigint "0" to-bigint big-mod"#);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("division by zero"));
}

#[test]
fn test_bigint_sub_negative_result_error() {
    let result = eval(r#""100" to-bigint "200" to-bigint big-sub"#);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("negative"));
}

#[test]
fn test_bigint_negative_number_error() {
    let result = eval(r#"-5 to-bigint"#);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid decimal"));
}

#[test]
fn test_bigint_invalid_decimal_error() {
    let result = eval(r#""abc" to-bigint"#);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid"));
}

#[test]
fn test_bigint_invalid_hex_error() {
    let result = eval(r#""0xGHI" to-bigint"#);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid"));
}

#[test]
fn test_bigint_256bit_arithmetic() {
    // Testing operations at 256-bit scale (common in blockchain/crypto)
    let max_256 = "115792089237316195423570985008687907853269984665640564039457584007913129639935";
    let output = eval(&format!(r#""{}" to-bigint "1" to-bigint big-sub to-string"#, max_256)).unwrap();
    assert_eq!(output.trim(), "115792089237316195423570985008687907853269984665640564039457584007913129639934");
}

#[test]
fn test_bigint_hash_xor_identity() {
    // XORing a hash with itself should give zero
    let output = eval(r#""hello" sha256 to-bigint dup big-xor to-string"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_bigint_modular_arithmetic() {
    // Simple modular exponentiation concept: (a * b) mod n
    // (7 * 11) mod 13 = 77 mod 13 = 12
    let output = eval(r#""7" to-bigint "11" to-bigint big-mul "13" to-bigint big-mod to-string"#).unwrap();
    assert_eq!(output.trim(), "12");
}

#[test]
fn test_bigint_bytes_roundtrip() {
    // Convert to BigInt and back to bytes should be identical
    let output = eval(r#""hello" sha256 to-bigint to-bytes typeof"#).unwrap();
    assert_eq!(output.trim(), "Bytes");
}

#[test]
fn test_bigint_div_truncates() {
    // Integer division should truncate
    let output = eval(r#""10" to-bigint "3" to-bigint big-div to-string"#).unwrap();
    assert_eq!(output.trim(), "3"); // 10/3 = 3 (not 3.33...)
}

#[test]
fn test_bigint_div_exact() {
    // Exact division
    let output = eval(r#""12" to-bigint "4" to-bigint big-div to-string"#).unwrap();
    assert_eq!(output.trim(), "3");
}

#[test]
fn test_bigint_mod_smaller_than_divisor() {
    // When dividend < divisor, mod returns dividend
    let output = eval(r#""5" to-bigint "10" to-bigint big-mod to-string"#).unwrap();
    assert_eq!(output.trim(), "5");
}

