//! Integration tests for table operations (issue #26):
//! group-by, join-on, add-column, map-column, rename-column, transpose,
//! first, last, sort-by-desc

#[path = "common/mod.rs"]
mod common;
#[allow(unused_imports)]
use common::{eval, eval_exit_code, lex, parse, Evaluator};

/// A three-row table: t=a/n=1, t=a/n=2, t=b/n=3
const TN_TABLE: &str =
    r#"marker "t" "a" "n" 1 record "t" "a" "n" 2 record "t" "b" "n" 3 record table"#;

/// A two-row people table (from-csv, so two tables can coexist on the stack)
const PEOPLE: &str = r#""id,name
1,alice
2,bob" from-csv"#;

/// A three-row orders table keyed by user id
const ORDERS: &str = r#""uid,item
1,book
1,pen
3,hat" from-csv"#;

// === group-by ===

#[test]
fn test_group_by_keys() {
    let output = eval(&format!(r#"{} "t" group-by keys"#, TN_TABLE)).unwrap();
    assert_eq!(output.trim(), "a\nb");
}

#[test]
fn test_group_by_subtable_rows() {
    // Group "a" holds two rows; count its "n" column values
    let output = eval(&format!(
        r#"{} "t" group-by "a" get "n" get count"#,
        TN_TABLE
    ))
    .unwrap();
    assert_eq!(output.trim(), "2");
}

#[test]
fn test_group_by_missing_column_errors() {
    let result = eval(&format!(r#"{} "zzz" group-by"#, TN_TABLE));
    assert!(result.is_err());
}

// === join-on ===

#[test]
fn test_join_on_inner_join_rows() {
    // alice (id 1) has two orders; bob (id 2) has none -> 2 joined rows
    let output = eval(&format!(
        r#"{} {} "id" "uid" join-on "item" get"#,
        PEOPLE, ORDERS
    ))
    .unwrap();
    assert_eq!(output.trim(), "book\npen");
}

#[test]
fn test_join_on_drops_right_key_column() {
    let output = eval(&format!(
        r#"{} {} "id" "uid" join-on 0 nth keys"#,
        PEOPLE, ORDERS
    ))
    .unwrap();
    let keys = output.trim();
    assert!(keys.contains("id"));
    assert!(keys.contains("name"));
    assert!(keys.contains("item"));
    assert!(!keys.contains("uid"));
}

#[test]
fn test_join_on_collision_suffixes_right_column() {
    // Both tables have a "name" column; the right one is suffixed
    let left = r#""id,name
1,alice" from-csv"#;
    let right = r#""rid,name
1,right-name" from-csv"#;
    let output = eval(&format!(
        r#"{} {} "id" "rid" join-on 0 nth keys"#,
        left, right
    ))
    .unwrap();
    assert!(output.contains("name_right"), "keys: {}", output);
}

#[test]
fn test_join_on_missing_key_errors() {
    let result = eval(&format!(r#"{} {} "nope" "uid" join-on"#, PEOPLE, ORDERS));
    assert!(result.is_err());
}

// === add-column ===

#[test]
fn test_add_column_computed_from_row() {
    let output = eval(&format!(
        r#"{} #["n" get 10 mul] "n10" add-column "n10" get"#,
        TN_TABLE
    ))
    .unwrap();
    assert_eq!(output.trim(), "10\n20\n30");
}

#[test]
fn test_add_column_appears_in_keys() {
    let output = eval(&format!(
        r#"{} #["t" get] "t2" add-column 0 nth keys"#,
        TN_TABLE
    ))
    .unwrap();
    let keys = output.trim();
    assert!(keys.contains("t2"));
    assert!(keys.ends_with("t2"), "new column should be last: {}", keys);
}

// === map-column ===

#[test]
fn test_map_column_transforms_in_place() {
    let output = eval(&format!(r#"{} "n" #[2 mul] map-column "n" get"#, TN_TABLE)).unwrap();
    assert_eq!(output.trim(), "2\n4\n6");
}

#[test]
fn test_map_column_leaves_other_columns() {
    let output = eval(&format!(r#"{} "n" #[2 mul] map-column "t" get"#, TN_TABLE)).unwrap();
    assert_eq!(output.trim(), "a\na\nb");
}

#[test]
fn test_map_column_missing_column_errors() {
    let result = eval(&format!(r#"{} "zzz" #[2 mul] map-column"#, TN_TABLE));
    assert!(result.is_err());
}

// === rename-column ===

#[test]
fn test_rename_column_renames() {
    let output = eval(&format!(
        r#"{} "n" "count" rename-column 0 nth keys"#,
        TN_TABLE
    ))
    .unwrap();
    let keys = output.trim();
    assert!(keys.contains("count"));
    assert!(!keys.split_whitespace().any(|k| k == "n"));
}

#[test]
fn test_rename_column_preserves_values() {
    let output = eval(&format!(
        r#"{} "n" "count" rename-column "count" get"#,
        TN_TABLE
    ))
    .unwrap();
    assert_eq!(output.trim(), "1\n2\n3");
}

#[test]
fn test_rename_column_missing_errors() {
    let result = eval(&format!(r#"{} "zzz" "count" rename-column"#, TN_TABLE));
    assert!(result.is_err());
}

// === transpose ===

#[test]
fn test_transpose_column_names_become_rows() {
    let output = eval(&format!(r#"{} transpose "column" get"#, TN_TABLE)).unwrap();
    assert_eq!(output.trim(), "t\nn");
}

#[test]
fn test_transpose_cells_move() {
    // Original row 1 (0-indexed) is t=a/n=2; transposed column "1" holds [a, 2]
    let output = eval(&format!(r#"{} transpose "1" get"#, TN_TABLE)).unwrap();
    assert_eq!(output.trim(), "a\n2");
}

// === first / last ===

#[test]
fn test_first_n_rows() {
    let output = eval(&format!(r#"{} 2 first "n" get"#, TN_TABLE)).unwrap();
    assert_eq!(output.trim(), "1\n2");
}

#[test]
fn test_first_more_than_rows() {
    let output = eval(&format!(r#"{} 10 first "n" get"#, TN_TABLE)).unwrap();
    assert_eq!(output.trim(), "1\n2\n3");
}

#[test]
fn test_last_n_rows() {
    let output = eval(&format!(r#"{} 2 last "n" get"#, TN_TABLE)).unwrap();
    assert_eq!(output.trim(), "2\n3");
}

#[test]
fn test_last_single_row() {
    let output = eval(&format!(r#"{} 1 last "t" get"#, TN_TABLE)).unwrap();
    assert_eq!(output.trim(), "b");
}

// === sort-by-desc ===

#[test]
fn test_sort_by_desc_numeric() {
    let output = eval(&format!(r#"{} "n" sort-by-desc "n" get"#, TN_TABLE)).unwrap();
    assert_eq!(output.trim(), "3\n2\n1");
}

#[test]
fn test_sort_by_desc_strings() {
    let output = eval(&format!(r#"{} "t" sort-by-desc "t" get"#, TN_TABLE)).unwrap();
    assert_eq!(output.trim(), "b\na\na");
}

#[test]
fn test_sort_by_desc_is_reverse_of_sort_by() {
    let asc = eval(&format!(r#"{} "n" sort-by "n" get"#, TN_TABLE)).unwrap();
    let desc = eval(&format!(r#"{} "n" sort-by-desc "n" get"#, TN_TABLE)).unwrap();
    let mut asc_lines: Vec<&str> = asc.trim().lines().collect();
    asc_lines.reverse();
    let desc_lines: Vec<&str> = desc.trim().lines().collect();
    assert_eq!(asc_lines, desc_lines);
}
