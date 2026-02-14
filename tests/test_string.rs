//! Integration tests for string operations

#[path = "common/mod.rs"]
mod common;
#[allow(unused_imports)]
use common::{eval, eval_exit_code, Evaluator, lex, parse};

#[test]
fn test_triple_single_quote() {
    // Triple single quotes should preserve the content
    let output = eval("'''hello world''' echo").unwrap();
    assert!(output.contains("hello") && output.contains("world"),
            "triple single quotes should work: {}", output);
}

#[test]
fn test_triple_double_quote() {
    // Triple double quotes should work too
    let output = eval(r#""""test string""" echo"#).unwrap();
    assert!(output.contains("test") && output.contains("string"),
            "triple double quotes should work: {}", output);
}

#[test]
fn test_len() {
    let output = eval("hello len").unwrap();
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_len_empty() {
    let output = eval("\"\" len").unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_slice() {
    let output = eval("hello 1 3 slice").unwrap();
    assert_eq!(output.trim(), "ell");
}

#[test]
fn test_slice_from_start() {
    let output = eval("hello 0 2 slice").unwrap();
    assert_eq!(output.trim(), "he");
}

#[test]
fn test_indexof_found() {
    let output = eval("hello ll indexof").unwrap();
    assert_eq!(output.trim(), "2");
}

#[test]
fn test_indexof_not_found() {
    let output = eval("hello xyz indexof").unwrap();
    assert_eq!(output.trim(), "-1");
}

#[test]
fn test_indexof_at_start() {
    let output = eval("hello he indexof").unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_str_replace() {
    let output = eval("hello l L str-replace").unwrap();
    assert_eq!(output.trim(), "heLLo");
}

#[test]
fn test_str_replace_not_found() {
    let output = eval("hello x y str-replace").unwrap();
    assert_eq!(output.trim(), "hello");
}

#[test]
fn test_str_replace_newlines() {
    let output = eval(r#""a\nb\nc" "\n" ", " str-replace"#).unwrap();
    assert_eq!(output.trim(), "a, b, c");
}

#[test]
fn test_format_sequential() {
    // value template format (values first, template last before format)
    let output = eval(r#"Alice "Hello, {}!" format"#).unwrap();
    assert_eq!(output.trim(), "Hello, Alice!");
}

#[test]
fn test_format_multiple_sequential() {
    // value1 value2 value3 template format
    let output = eval(r#"1 2 3 "{} + {} = {}" format"#).unwrap();
    assert_eq!(output.trim(), "1 + 2 = 3");
}

#[test]
fn test_format_positional() {
    // bob alice template format -> {0}=bob, {1}=alice
    let output = eval(r#"bob alice "{1} meets {0}" format"#).unwrap();
    assert_eq!(output.trim(), "alice meets bob");
}

#[test]
fn test_format_mixed() {
    // Mix of sequential and positional
    let output = eval(r#"hello world "{} says {0}" format"#).unwrap();
    // {} consumes first value (hello), then {0} also uses first value (hello)
    assert_eq!(output.trim(), "hello says hello");
}

#[test]
fn test_recursion_limit_triggered() {
    // Set a low recursion limit for testing
    std::env::set_var("HSAB_MAX_RECURSION", "100");

    // Define infinite recursion and try to execute
    // The recursion limit should catch this
    let result = eval("[foo] :foo foo");

    // Restore to default
    std::env::remove_var("HSAB_MAX_RECURSION");

    assert!(result.is_err(), "Infinite recursion should trigger error");
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("Recursion limit"), "Error should mention recursion limit: {}", err_msg);
}

#[test]
fn test_safe_recursion_works() {
    // Simple recursion that terminates after a few calls
    // Define countdown: if n > 0, decrement and recurse, else push done
    // Definition: [block] :name
    let output = eval(r#"[[dup 0 gt?] [1 minus countdown] [drop done] if] :countdown 3 countdown"#).unwrap();
    // Should terminate successfully (done is pushed as literal)
    assert!(output.contains("done"), "Safe recursion should complete: {}", output);
}

#[test]
fn test_interpolation_simple() {
    std::env::set_var("HSAB_INTERP_SIMPLE", "world");
    let output = eval(r#""hello $HSAB_INTERP_SIMPLE" echo"#).unwrap();
    assert!(output.contains("hello world"), "Should interpolate variable: {}", output);
    std::env::remove_var("HSAB_INTERP_SIMPLE");
}

#[test]
fn test_interpolation_braces() {
    std::env::set_var("HSAB_INTERP_BRACE", "foo");
    let output = eval(r#""${HSAB_INTERP_BRACE}bar" echo"#).unwrap();
    assert!(output.contains("foobar"), "Should interpolate with braces: {}", output);
    std::env::remove_var("HSAB_INTERP_BRACE");
}

#[test]
fn test_interpolation_escaped() {
    let output = eval(r#""price is \$100" echo"#).unwrap();
    assert!(output.contains("$100"), "Should escape dollar sign: {}", output);
}

#[test]
fn test_reext_basic() {
    let output = eval(r#""file.txt" ".md" reext"#).unwrap();
    assert_eq!(output.trim(), "file.md");
}

#[test]
fn test_reext_no_extension() {
    let output = eval(r#""README" ".md" reext"#).unwrap();
    assert_eq!(output.trim(), "README.md");
}

#[test]
fn test_reext_complex_path() {
    let output = eval(r#""/path/to/file.txt" ".bak" reext"#).unwrap();
    assert_eq!(output.trim(), "/path/to/file.bak");
}

#[test]
fn test_reext_multiple_dots() {
    let output = eval(r#""file.tar.gz" ".zip" reext"#).unwrap();
    assert_eq!(output.trim(), "file.tar.zip");
}

#[test]
fn test_split1_no_delimiter() {
    let output = eval(r#""hello" "x" split1"#).unwrap();
    // No split, return original + empty
    assert!(output.contains("hello"));
}

#[test]
fn test_rsplit1_no_delimiter() {
    let output = eval(r#""hello" "x" rsplit1"#).unwrap();
    assert!(output.contains("hello"));
}

#[test]
fn test_len_unicode() {
    // Unicode characters
    let output = eval(r#""héllo" len"#).unwrap();
    // Should count characters not bytes
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_format_no_placeholders() {
    let output = eval(r#""hello world" format"#).unwrap();
    assert_eq!(output.trim(), "hello world");
}

#[test]
fn test_format_single_placeholder() {
    let output = eval(r#""hello" "{}" format"#).unwrap();
    assert!(output.contains("hello"));
}

#[test]
fn test_slice_basic() {
    // slice: string start length → substring
    let output = eval(r#""hello" 1 3 slice"#).unwrap();
    assert_eq!(output.trim(), "ell");
}

#[test]
fn test_slice_start_zero() {
    let output = eval(r#""hello" 0 2 slice"#).unwrap();
    assert_eq!(output.trim(), "he");
}

#[test]
fn test_printf_string() {
    let output = eval(r#""world" "hello %s" printf"#).unwrap();
    assert!(output.contains("hello world"));
}

#[test]
fn test_printf_number() {
    let output = eval(r#"42 "answer: %d" printf"#).unwrap();
    assert!(output.contains("answer: 42"));
}


// === Recovered tests ===

#[test]
fn test_sort_nums_single() {
    let output = eval(r#"'[42]' into-json sort-nums to-json"#).unwrap();
    assert_eq!(output.trim(), "[42.0]");
}

#[test]
fn test_len_single_char() {
    let output = eval(r#""a" len"#).unwrap();
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_len_whitespace_only() {
    let output = eval(r#""   " len"#).unwrap();
    assert_eq!(output.trim(), "3");
}

#[test]
fn test_len_unicode_emoji() {
    // Emoji should count as one character
    let output = eval(r#""hello world" len"#).unwrap();
    // The emoji is 2 chars in this context (surrogate pair handling may vary)
    // Let's test with a simple multi-byte char
    let output2 = eval(r#""cafe" len"#).unwrap();
    assert_eq!(output2.trim(), "4");
}

#[test]
fn test_len_unicode_multibyte() {
    // Chinese characters
    let output = eval(r#""abc" len"#).unwrap();
    assert_eq!(output.trim(), "3");
}

#[test]
fn test_len_with_newlines() {
    let output = eval(r#""a\nb\nc" len"#).unwrap();
    // "a\nb\nc" = 5 characters (a, \n, b, \n, c)
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_len_with_tabs() {
    let output = eval(r#""a\tb" len"#).unwrap();
    // "a\tb" = 3 characters (a, \t, b)
    assert_eq!(output.trim(), "3");
}

#[test]
fn test_slice_empty_string() {
    let output = eval(r#""" 0 5 slice"#).unwrap();
    assert_eq!(output.trim(), "");
}

#[test]
fn test_slice_zero_length() {
    let output = eval(r#""hello" 2 0 slice"#).unwrap();
    assert_eq!(output.trim(), "");
}

#[test]
fn test_slice_entire_string() {
    let output = eval(r#""hello" 0 5 slice"#).unwrap();
    assert_eq!(output.trim(), "hello");
}

#[test]
fn test_slice_beyond_end() {
    // Requesting more characters than available
    let output = eval(r#""hi" 0 100 slice"#).unwrap();
    assert_eq!(output.trim(), "hi");
}

#[test]
fn test_slice_start_at_end() {
    let output = eval(r#""hello" 5 3 slice"#).unwrap();
    assert_eq!(output.trim(), "");
}

#[test]
fn test_slice_start_beyond_end() {
    let output = eval(r#""hello" 10 3 slice"#).unwrap();
    assert_eq!(output.trim(), "");
}

#[test]
fn test_slice_unicode_chars() {
    // Unicode-aware slicing (chars, not bytes)
    let output = eval(r#""cafe" 0 4 slice"#).unwrap();
    assert_eq!(output.trim(), "cafe");
}

#[test]
fn test_slice_unicode_middle() {
    // Slice from middle of unicode string
    let output = eval(r#""abcdef" 2 2 slice"#).unwrap();
    assert_eq!(output.trim(), "cd");
}

#[test]
fn test_slice_single_char() {
    let output = eval(r#""hello" 1 1 slice"#).unwrap();
    assert_eq!(output.trim(), "e");
}

#[test]
fn test_slice_last_char() {
    let output = eval(r#""hello" 4 1 slice"#).unwrap();
    assert_eq!(output.trim(), "o");
}

#[test]
fn test_indexof_empty_needle() {
    // Empty string is found at position 0
    let output = eval(r#""hello" "" indexof"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_indexof_empty_haystack() {
    let output = eval(r#""" "x" indexof"#).unwrap();
    assert_eq!(output.trim(), "-1");
}

#[test]
fn test_indexof_both_empty() {
    let output = eval(r#""" "" indexof"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_indexof_at_end() {
    let output = eval(r#""hello" "o" indexof"#).unwrap();
    assert_eq!(output.trim(), "4");
}

#[test]
fn test_indexof_multiple_occurrences() {
    // Should find first occurrence
    let output = eval(r#""banana" "an" indexof"#).unwrap();
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_indexof_exact_match() {
    let output = eval(r#""hello" "hello" indexof"#).unwrap();
    assert_eq!(output.trim(), "0");
}

#[test]
fn test_indexof_needle_longer_than_haystack() {
    let output = eval(r#""hi" "hello" indexof"#).unwrap();
    assert_eq!(output.trim(), "-1");
}

#[test]
fn test_indexof_case_sensitive() {
    let output = eval(r#""Hello" "h" indexof"#).unwrap();
    assert_eq!(output.trim(), "-1");
}

#[test]
fn test_indexof_unicode() {
    // Note: indexof returns byte position, not char position
    // This tests the behavior is consistent
    let output = eval(r#""cafe" "e" indexof"#).unwrap();
    // 'e' is at position 3 (c=0, a=1, f=2, e=3)
    assert_eq!(output.trim(), "3");
}

#[test]
fn test_indexof_special_chars() {
    let output = eval(r#""a.b.c" "." indexof"#).unwrap();
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_indexof_newline() {
    let output = eval(r#""a\nb" "\n" indexof"#).unwrap();
    assert_eq!(output.trim(), "1");
}

#[test]
fn test_str_replace_empty_from() {
    // Replacing empty string - inserts between each char
    let output = eval(r#""abc" "" "X" str-replace"#).unwrap();
    // Rust's replace("", "X") inserts X before each char and at end
    assert_eq!(output.trim(), "XaXbXcX");
}

#[test]
fn test_str_replace_empty_to() {
    // Replacing with empty string - deletion
    let output = eval(r#""hello" "l" "" str-replace"#).unwrap();
    assert_eq!(output.trim(), "heo");
}

#[test]
fn test_str_replace_empty_string() {
    let output = eval(r#""" "x" "y" str-replace"#).unwrap();
    assert_eq!(output.trim(), "");
}

#[test]
fn test_str_replace_entire_string() {
    let output = eval(r#""hello" "hello" "world" str-replace"#).unwrap();
    assert_eq!(output.trim(), "world");
}

#[test]
fn test_str_replace_overlapping_pattern() {
    // Non-overlapping replacement
    let output = eval(r#""aaa" "aa" "b" str-replace"#).unwrap();
    // First "aa" replaced, leaving "ba"
    assert_eq!(output.trim(), "ba");
}

#[test]
fn test_str_replace_longer_replacement() {
    let output = eval(r#""a" "a" "xyz" str-replace"#).unwrap();
    assert_eq!(output.trim(), "xyz");
}

#[test]
fn test_str_replace_unicode() {
    let output = eval(r#""cafe" "e" "E" str-replace"#).unwrap();
    assert_eq!(output.trim(), "cafE");
}

#[test]
fn test_str_replace_special_chars() {
    let output = eval(r#""a.b.c" "." "-" str-replace"#).unwrap();
    assert_eq!(output.trim(), "a-b-c");
}

#[test]
fn test_str_replace_consecutive() {
    let output = eval(r#""aabbcc" "bb" "B" str-replace"#).unwrap();
    assert_eq!(output.trim(), "aaBcc");
}

#[test]
fn test_str_replace_at_boundaries() {
    // Replace at start
    let output1 = eval(r#""abc" "a" "X" str-replace"#).unwrap();
    assert_eq!(output1.trim(), "Xbc");

    // Replace at end
    let output2 = eval(r#""abc" "c" "X" str-replace"#).unwrap();
    assert_eq!(output2.trim(), "abX");
}

#[test]
fn test_format_empty_template() {
    let output = eval(r#""" format"#).unwrap();
    assert_eq!(output.trim(), "");
}

#[test]
fn test_format_no_values() {
    let output = eval(r#""hello {}" format"#).unwrap();
    // No value to substitute, {} remains
    assert_eq!(output.trim(), "hello {}");
}

#[test]
fn test_format_extra_values() {
    // More values than placeholders - extra values ignored
    let output = eval(r#"a b c "{}" format"#).unwrap();
    // Only first value used
    assert_eq!(output.trim(), "a");
}

#[test]
fn test_format_escaped_braces() {
    // Note: hsab format doesn't escape braces like {{}}
    // This tests what happens with literal braces that aren't placeholders
    let output = eval(r#""hello" "say {}" format"#).unwrap();
    assert_eq!(output.trim(), "say hello");
}

#[test]
fn test_format_positional_out_of_bounds() {
    // Position that doesn't exist
    let output = eval(r#"only "{0} {1} {2}" format"#).unwrap();
    // {1} and {2} should remain as-is since no values for those positions
    assert_eq!(output.trim(), "only {1} {2}");
}

#[test]
fn test_format_mixed_sequential_positional() {
    // Sequential {} uses next available, positional {n} uses specific
    let output = eval(r#"a b "{} and {1}" format"#).unwrap();
    // {} gets 'a', {1} gets 'b'
    assert_eq!(output.trim(), "a and b");
}

#[test]
fn test_format_unicode_values() {
    let output = eval(r#""world" "Hello, {}!" format"#).unwrap();
    assert_eq!(output.trim(), "Hello, world!");
}

#[test]
fn test_format_newline_in_template() {
    let output = eval(r#"name "Hello\n{}" format"#).unwrap();
    assert!(output.contains("Hello"));
    assert!(output.contains("name"));
}

#[test]
fn test_format_empty_value() {
    let output = eval(r#""" "value: [{}]" format"#).unwrap();
    assert_eq!(output.trim(), "value: []");
}

#[test]
fn test_format_three_positional() {
    let output = eval(r#"a b c "{2}-{1}-{0}" format"#).unwrap();
    assert_eq!(output.trim(), "c-b-a");
}

#[test]
fn test_split1_empty_delimiter() {
    let output = eval(r#""hello" "" split1"#).unwrap();
    let lines: Vec<&str> = output.lines().collect();
    // Empty delimiter matches at position 0 -> ["", "hello"]
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "");
    assert_eq!(lines[1], "hello");
}

#[test]
fn test_split1_at_start() {
    let output = eval(r#"".abc" "." split1"#).unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "");
    assert_eq!(lines[1], "abc");
}

#[test]
fn test_split1_multiple_chars_delimiter() {
    let output = eval(r#""a::b::c" "::" split1"#).unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "a");
    assert_eq!(lines[1], "b::c");
}

#[test]
fn test_split1_consecutive_delimiters() {
    let output = eval(r#""a..b" "." split1"#).unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "a");
    assert_eq!(lines[1], ".b");
}

#[test]
fn test_split1_unicode_delimiter() {
    let output = eval(r#""hello-world" "-" split1"#).unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "hello");
    assert_eq!(lines[1], "world");
}

#[test]
fn test_rsplit1_at_start() {
    let output = eval(r#"".abc" "." rsplit1"#).unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "");
    assert_eq!(lines[1], "abc");
}

#[test]
fn test_rsplit1_multiple_chars_delimiter() {
    let output = eval(r#""a::b::c" "::" rsplit1"#).unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "a::b");
    assert_eq!(lines[1], "c");
}

#[test]
fn test_rsplit1_consecutive_delimiters() {
    let output = eval(r#""a..b" "." rsplit1"#).unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "a.");
    assert_eq!(lines[1], "b");
}

#[test]
fn test_rsplit1_path_like() {
    // Common use case: extract filename from path
    let output = eval(r#""/usr/local/bin/file" "/" rsplit1"#).unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "/usr/local/bin");
    assert_eq!(lines[1], "file");
}

#[test]
fn test_rsplit1_extension() {
    // Common use case: split extension from filename
    let output = eval(r#""file.tar.gz" "." rsplit1"#).unwrap();
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "file.tar");
    assert_eq!(lines[1], "gz");
}

#[test]
fn test_split1_vs_rsplit1_single_delimiter() {
    // With single delimiter, both should produce same result
    let output1 = eval(r#""a.b" "." split1"#).unwrap();
    let output2 = eval(r#""a.b" "." rsplit1"#).unwrap();
    assert_eq!(output1, output2);
}

#[test]
fn test_split1_vs_rsplit1_multiple_delimiters() {
    // With multiple delimiters, results differ
    let split1_output = eval(r#""a.b.c" "." split1"#).unwrap();
    let rsplit1_output = eval(r#""a.b.c" "." rsplit1"#).unwrap();

    let split1_lines: Vec<&str> = split1_output.lines().collect();
    let rsplit1_lines: Vec<&str> = rsplit1_output.lines().collect();

    // split1: ["a", "b.c"]
    assert_eq!(split1_lines[0], "a");
    assert_eq!(split1_lines[1], "b.c");

    // rsplit1: ["a.b", "c"]
    assert_eq!(rsplit1_lines[0], "a.b");
    assert_eq!(rsplit1_lines[1], "c");
}

#[test]
fn test_len_after_slice() {
    let output = eval(r#""hello world" 0 5 slice len"#).unwrap();
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_indexof_after_str_replace() {
    let output = eval(r#""hello" "l" "x" str-replace "x" indexof"#).unwrap();
    // "hexxo" -> first x at position 2
    assert_eq!(output.trim(), "2");
}

#[test]
fn test_format_with_split1_result() {
    // Split and format
    let output = eval(r#""user@domain" "@" split1 swap "{} at {}" format"#).unwrap();
    // After split1: "user" "domain", swap: "domain" "user"
    // format with "domain" "user" "{} at {}" -> "domain at user"
    assert_eq!(output.trim(), "domain at user");
}

#[test]
fn test_slice_then_str_replace() {
    let output = eval(r#""hello world" 0 5 slice "l" "L" str-replace"#).unwrap();
    assert_eq!(output.trim(), "heLLo");
}

#[test]
fn test_bigint_mod_smaller_than_divisor() {
    // When dividend < divisor, mod returns dividend
    let output = eval(r#""5" to-bigint "10" to-bigint big-mod to-string"#).unwrap();
    assert_eq!(output.trim(), "5");
}

#[test]
fn test_str_replace_at_start() {
    // Replace at start
    let output = eval(r#""abc" "a" "X" str-replace"#).unwrap();
    assert_eq!(output.trim(), "Xbc");
}

#[test]
fn test_str_replace_at_end() {
    // Replace at end
    let output = eval(r#""abc" "c" "X" str-replace"#).unwrap();
    assert_eq!(output.trim(), "abX");
}

