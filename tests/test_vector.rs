//! Integration tests for vector operations

#[path = "common/mod.rs"]
mod common;
#[allow(unused_imports)]
use common::{eval, eval_exit_code, Evaluator, lex, parse};

#[test]
fn test_dot_product() {
    // [1,2,3] · [4,5,6] = 1*4 + 2*5 + 3*6 = 4 + 10 + 18 = 32
    let output = eval(r#"'[1,2,3]' json '[4,5,6]' json dot-product"#).unwrap();
    assert_eq!(output.trim(), "32");
}

#[test]
fn test_magnitude() {
    // |[3,4]| = sqrt(9 + 16) = 5
    let output = eval(r#"'[3,4]' json magnitude"#).unwrap();
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_magnitude_3d() {
    // |[1,2,2]| = sqrt(1 + 4 + 4) = 3
    let output = eval(r#"'[1,2,2]' json magnitude"#).unwrap();
    assert_eq!(output.trim(), "3");
}

#[test]
fn test_normalize() {
    // normalize [3,4] = [0.6, 0.8]
    let output = eval(r#"'[3,4]' json normalize to-json"#).unwrap();
    assert!(output.contains("0.6") && output.contains("0.8"), "Should be unit vector: {}", output);
}

#[test]
fn test_normalize_zero_vector() {
    // normalize [0,0] = [0,0]
    let output = eval(r#"'[0,0]' json normalize to-json"#).unwrap();
    assert!(output.contains("0"), "Zero vector should stay zero: {}", output);
}

#[test]
fn test_cosine_similarity_identical() {
    // cos([1,0], [1,0]) = 1
    let output = eval(r#"'[1,0]' json '[1,0]' json cosine-similarity"#).unwrap();
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_cosine_similarity_orthogonal() {
    // cos([1,0], [0,1]) = 0
    let output = eval(r#"'[1,0]' json '[0,1]' json cosine-similarity"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_cosine_similarity_opposite() {
    // cos([1,0], [-1,0]) = -1
    let output = eval(r#"'[1,0]' json '[-1,0]' json cosine-similarity"#).unwrap();
    assert_eq!(output.trim(), "-1");
}

#[test]
fn test_euclidean_distance() {
    // dist([0,0], [3,4]) = 5
    let output = eval(r#"'[0,0]' json '[3,4]' json euclidean-distance"#).unwrap();
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_euclidean_distance_same() {
    // dist([1,2], [1,2]) = 0
    let output = eval(r#"'[1,2]' json '[1,2]' json euclidean-distance"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_vector_ops_length_mismatch() {
    // Different length vectors should error
    let result = eval(r#"'[1,2,3]' json '[1,2]' json dot-product"#);
    assert!(result.is_err(), "Should error on length mismatch");
}

