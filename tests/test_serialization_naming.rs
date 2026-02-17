//! Integration tests for standardized serialization naming
//! into-X = serialize (structured -> text)
//! from-X = parse (text -> structured)

#[path = "common/mod.rs"]
mod common;
use common::eval;

// === JSON ===

#[test]
fn test_into_json_serializes_record() {
    // into-json should serialize a value to JSON string (same as old to-json)
    let output = eval(r#""name" "Alice" "age" "30" record into-json"#).unwrap();
    assert!(output.contains("Alice"), "into-json should serialize record to JSON: {}", output);
    assert!(output.contains("name"), "into-json should contain field name: {}", output);
}

#[test]
fn test_into_json_serializes_list() {
    // into-json on a list
    let output = eval(r#"'[1,2,3]' json into-json"#).unwrap();
    assert!(output.contains("[") && output.contains("1") && output.contains("2") && output.contains("3"),
        "into-json should serialize list: {}", output);
}

#[test]
fn test_to_json_still_works_as_alias() {
    // to-json should still work (alias for into-json serialize)
    let output = eval(r#""name" "test" record to-json"#).unwrap();
    assert!(output.contains("name") && output.contains("test"),
        "to-json should still work as alias: {}", output);
}

#[test]
fn test_from_json_parses_object() {
    // from-json should parse a JSON string into structured data
    let output = eval(r#"'{"a":1}' from-json keys"#).unwrap();
    assert!(output.contains("a"), "from-json should parse JSON object: {}", output);
}

#[test]
fn test_from_json_parses_array() {
    // from-json should parse a JSON array
    let output = eval(r#"'[1,2,3]' from-json typeof"#).unwrap();
    assert_eq!(output.trim(), "list", "from-json should parse JSON array to list");
}

#[test]
fn test_from_json_parses_number() {
    let output = eval(r#"'42' from-json typeof"#).unwrap();
    assert_eq!(output.trim(), "number", "from-json should parse JSON number");
}

// === CSV ===

#[test]
fn test_into_csv_serializes_table() {
    // into-csv should serialize table to CSV string (same as old to-csv)
    let output = eval(r#"
        marker
            "name" "alice" "age" "30" record
            "name" "bob" "age" "25" record
        table
        into-csv
    "#).unwrap();
    assert!(output.contains("name") && output.contains("age"),
        "into-csv should have CSV headers: {}", output);
    assert!(output.contains("alice") && output.contains("bob"),
        "into-csv should have CSV data: {}", output);
}

#[test]
fn test_to_csv_still_works_as_alias() {
    let output = eval(r#"
        marker "name" "alice" record table to-csv
    "#).unwrap();
    assert!(output.contains("name") && output.contains("alice"),
        "to-csv should still work: {}", output);
}

#[test]
fn test_from_csv_parses_csv_string() {
    // from-csv should parse a CSV string into a table
    let output = eval(r#""name,age\nalice,30\nbob,25" from-csv typeof"#).unwrap();
    assert_eq!(output.trim(), "table", "from-csv should produce a table");
}

#[test]
fn test_from_csv_correct_data() {
    let output = eval(r#""name,age\nalice,30" from-csv 0 nth "name" get"#).unwrap();
    assert_eq!(output.trim(), "alice", "from-csv should parse data correctly");
}

// === TSV ===

#[test]
fn test_into_tsv_serializes_table() {
    // into-tsv should serialize table to TSV string (same as old to-tsv)
    let output = eval(r#"
        marker
            "name" "alice" "age" "30" record
        table
        into-tsv
    "#).unwrap();
    assert!(output.contains("\t"), "into-tsv should produce tab-separated output: {}", output);
    assert!(output.contains("alice"), "into-tsv should contain data: {}", output);
}

#[test]
fn test_to_tsv_still_works_as_alias() {
    let output = eval(r#"
        marker "name" "alice" record table to-tsv
    "#).unwrap();
    assert!(output.contains("\t") || output.contains("alice"),
        "to-tsv should still work: {}", output);
}

#[test]
fn test_from_tsv_parses_tsv_string() {
    // from-tsv should parse a TSV string into a table
    let output = eval(r#""name\tage\nalice\t30" from-tsv typeof"#).unwrap();
    assert_eq!(output.trim(), "table", "from-tsv should produce a table");
}

#[test]
fn test_from_tsv_correct_data() {
    let output = eval(r#""name\tage\nalice\t30\nbob\t25" from-tsv count"#).unwrap();
    assert_eq!(output.trim(), "2", "from-tsv should parse 2 rows");
}

// === KV ===

#[test]
fn test_into_kv_serializes_record() {
    // into-kv should serialize record to key=value format (same as old to-kv)
    let output = eval(r#""name" "alice" "age" "30" record into-kv"#).unwrap();
    assert!(output.contains("age=30"), "into-kv should produce key=value: {}", output);
    assert!(output.contains("name=alice"), "into-kv should produce key=value: {}", output);
}

#[test]
fn test_to_kv_still_works_as_alias() {
    let output = eval(r#""name" "test" record to-kv"#).unwrap();
    assert!(output.contains("name=test"),
        "to-kv should still work: {}", output);
}

#[test]
fn test_from_kv_parses_kv_string() {
    // from-kv should parse key=value format into a record
    let output = eval(r#""name=test\nversion=1.0" from-kv typeof"#).unwrap();
    assert_eq!(output.trim(), "record", "from-kv should produce a record");
}

#[test]
fn test_from_kv_correct_data() {
    let output = eval(r#""name=test\nversion=1.0" from-kv "name" get"#).unwrap();
    assert_eq!(output.trim(), "test", "from-kv should parse data correctly");
}

// === Lines ===

#[test]
fn test_into_lines_still_parses() {
    // into-lines should still work as before (parse: string -> list)
    let output = eval(r#""a\nb\nc" into-lines count"#).unwrap();
    assert_eq!(output.trim(), "3", "into-lines should still split string into list");
}

#[test]
fn test_from_lines_parses() {
    // from-lines should be an alias for into-lines (parse: string -> list)
    let output = eval(r#""a\nb\nc" from-lines count"#).unwrap();
    assert_eq!(output.trim(), "3", "from-lines should split string into list");
}

#[test]
fn test_to_lines_still_serializes() {
    // to-lines should still serialize list to string
    let output = eval(r#"'["a","b","c"]' json to-lines"#).unwrap();
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines.len(), 3, "to-lines should produce 3 lines");
}

// === Round-trip tests ===

#[test]
fn test_json_roundtrip_from_into() {
    // from-json then into-json should roundtrip
    let output = eval(r#"'{"x":1}' from-json into-json"#).unwrap();
    assert!(output.contains("x") && output.contains("1"),
        "JSON roundtrip should preserve data: {}", output);
}

#[test]
fn test_csv_roundtrip_from_into() {
    // from-csv then into-csv should roundtrip
    let output = eval(r#""name,age\nalice,30" from-csv into-csv"#).unwrap();
    assert!(output.contains("name") && output.contains("alice"),
        "CSV roundtrip should preserve data: {}", output);
}

#[test]
fn test_kv_roundtrip_from_into() {
    // from-kv then into-kv should roundtrip
    let output = eval(r#""name=alice" from-kv into-kv"#).unwrap();
    assert!(output.contains("name=alice"),
        "KV roundtrip should preserve data: {}", output);
}
