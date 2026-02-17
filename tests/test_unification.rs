#[path = "common/mod.rs"]
mod common;
use common::eval;

#[test]
fn test_double_slash_comment() {
    assert_eq!(eval("42 // comment").unwrap().trim(), "42");
}
#[test]
fn test_double_slash_full_line() {
    assert_eq!(eval("// full comment\n5 3 plus").unwrap().trim(), "8");
}
#[test]
fn test_dupe_alias() {
    // 5 dupe -> [5, 5], dup -> [5, 5, 5], plus -> [5, 10]
    let output = eval("5 dupe dup plus").unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines, vec!["5", "10"]);
}
#[test]
fn test_count_alias() {
    // 1 2 3 count -> [1, 2, 3, 3]
    let output = eval("1 2 3 count").unwrap();
    assert!(output.contains("3"));
    assert_eq!(output.lines().count(), 4);
}
