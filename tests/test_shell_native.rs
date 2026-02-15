//! Tests for stack-native shell operations
//!
//! These operations return useful values instead of being side-effect only.

use std::fs;
use std::path::Path;

mod common;
use common::eval;

// ============================================
// File Creation
// ============================================

#[test]
fn test_touch_returns_path() {
    let tmp = std::env::temp_dir().join("hsab_test_touch.txt");
    let _ = fs::remove_file(&tmp); // Clean up first

    let cmd = format!("\"{}\" touch", tmp.display());
    let output = eval(&cmd).unwrap();

    // Should return the path
    assert!(output.contains(&tmp.to_string_lossy().to_string()) ||
            output.contains("hsab_test_touch.txt"));

    // File should exist
    assert!(tmp.exists());

    // Clean up
    let _ = fs::remove_file(&tmp);
}

#[test]
fn test_touch_error_returns_nil() {
    // Try to touch in a non-existent directory
    let output = eval("\"/nonexistent/path/file.txt\" touch").unwrap();
    assert!(output.contains("nil") || output.is_empty());
}

#[test]
fn test_mkdir_returns_path() {
    let tmp = std::env::temp_dir().join("hsab_test_mkdir");
    let _ = fs::remove_dir_all(&tmp); // Clean up first

    let cmd = format!("\"{}\" mkdir", tmp.display());
    let output = eval(&cmd).unwrap();

    // Should return the path
    assert!(output.contains("hsab_test_mkdir"));

    // Directory should exist
    assert!(tmp.is_dir());

    // Clean up
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_mkdir_p_creates_parents() {
    let tmp = std::env::temp_dir().join("hsab_test_mkdir_p/nested/deep");
    let parent = std::env::temp_dir().join("hsab_test_mkdir_p");
    let _ = fs::remove_dir_all(&parent); // Clean up first

    let cmd = format!("\"{}\" mkdir-p", tmp.display());
    let output = eval(&cmd).unwrap();

    // Should return the path
    assert!(output.contains("deep"));

    // All directories should exist
    assert!(tmp.is_dir());

    // Clean up
    let _ = fs::remove_dir_all(&parent);
}

#[test]
fn test_mktemp_returns_path() {
    let output = eval("mktemp").unwrap();

    // Should return a path
    assert!(output.contains("tmp") || output.contains("temp") || output.contains("var"));

    // File should exist
    let path = output.trim();
    if !path.is_empty() && !path.contains("nil") {
        assert!(Path::new(path).exists());
        let _ = fs::remove_file(path);
    }
}

#[test]
fn test_mktemp_d_returns_dir_path() {
    let output = eval("mktemp-d").unwrap();

    // Should return a path
    assert!(output.contains("tmp") || output.contains("temp") || output.contains("var"));

    // Directory should exist
    let path = output.trim();
    if !path.is_empty() && !path.contains("nil") {
        assert!(Path::new(path).is_dir());
        let _ = fs::remove_dir(path);
    }
}

// ============================================
// File Operations
// ============================================

#[test]
fn test_cp_returns_dest_path() {
    let src = std::env::temp_dir().join("hsab_test_cp_src.txt");
    let dst = std::env::temp_dir().join("hsab_test_cp_dst.txt");

    // Create source file
    fs::write(&src, "test content").unwrap();
    let _ = fs::remove_file(&dst);

    let cmd = format!("\"{}\" \"{}\" cp", src.display(), dst.display());
    let output = eval(&cmd).unwrap();

    // Should return dest path
    assert!(output.contains("hsab_test_cp_dst.txt"));

    // Both files should exist
    assert!(src.exists());
    assert!(dst.exists());

    // Content should match
    assert_eq!(fs::read_to_string(&dst).unwrap(), "test content");

    // Clean up
    let _ = fs::remove_file(&src);
    let _ = fs::remove_file(&dst);
}

#[test]
fn test_mv_returns_dest_path() {
    let src = std::env::temp_dir().join("hsab_test_mv_src.txt");
    let dst = std::env::temp_dir().join("hsab_test_mv_dst.txt");

    // Create source file
    fs::write(&src, "test content").unwrap();
    let _ = fs::remove_file(&dst);

    let cmd = format!("\"{}\" \"{}\" mv", src.display(), dst.display());
    let output = eval(&cmd).unwrap();

    // Should return dest path
    assert!(output.contains("hsab_test_mv_dst.txt"));

    // Source should be gone, dest should exist
    assert!(!src.exists());
    assert!(dst.exists());

    // Clean up
    let _ = fs::remove_file(&dst);
}

#[test]
fn test_rm_returns_count() {
    let tmp = std::env::temp_dir().join("hsab_test_rm.txt");
    fs::write(&tmp, "test").unwrap();

    let cmd = format!("\"{}\" rm", tmp.display());
    let output = eval(&cmd).unwrap();

    // Should return count (1)
    assert!(output.trim() == "1" || output.contains("1"));

    // File should be gone
    assert!(!tmp.exists());
}

#[test]
fn test_rm_nonexistent_returns_nil() {
    let output = eval("\"/nonexistent/file.txt\" rm").unwrap();
    assert!(output.contains("nil") || output.trim() == "0" || output.is_empty());
}

#[test]
fn test_realpath_returns_canonical() {
    // Use a known path
    let output = eval("\"/tmp\" realpath").unwrap();

    // On macOS, /tmp -> /private/tmp
    let path = output.trim();
    assert!(!path.is_empty());
    assert!(Path::new(path).is_absolute());
}

// ============================================
// Directory Operations
// ============================================

#[test]
fn test_cd_returns_path() {
    let output = eval("\"/tmp\" cd").unwrap();

    // Should return the canonical path
    assert!(output.contains("tmp") || output.contains("private"));
}

#[test]
fn test_which_returns_path() {
    let output = eval("\"ls\" which").unwrap();

    // Should return a path to ls
    assert!(output.contains("ls") || output.contains("bin"));
}

#[test]
fn test_which_not_found_returns_nil() {
    let output = eval("\"nonexistent_command_xyz\" which").unwrap();
    assert!(output.contains("nil") || output.is_empty());
}

// ============================================
// Path Parts
// ============================================

#[test]
fn test_dirname_extracts_directory() {
    let output = eval("\"/path/to/file.txt\" dirname").unwrap();
    assert_eq!(output.trim(), "/path/to");
}

#[test]
fn test_basename_extracts_stem() {
    // Note: hsab's basename returns the stem (filename without extension)
    // This differs from shell basename which returns the full filename
    let output = eval("\"/path/to/file.txt\" basename").unwrap();
    assert_eq!(output.trim(), "file");
}

#[test]
fn test_extname_extracts_extension() {
    let output = eval("\"/path/to/file.txt\" extname").unwrap();
    assert_eq!(output.trim(), ".txt");
}

#[test]
fn test_extname_no_extension_returns_empty() {
    let output = eval("\"/path/to/file\" extname").unwrap();
    assert!(output.trim().is_empty() || output.trim() == "\"\"");
}

// ============================================
// Enhanced Listing
// ============================================

#[test]
fn test_ls_returns_vector() {
    // Create a temp dir with known files
    let tmp = std::env::temp_dir().join("hsab_test_ls");
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    fs::write(tmp.join("a.txt"), "").unwrap();
    fs::write(tmp.join("b.txt"), "").unwrap();

    let cmd = format!("\"{}\" ls", tmp.display());
    let output = eval(&cmd).unwrap();

    // Should contain the files (as a list or individual items)
    assert!(output.contains("a.txt") || output.contains("b.txt"));

    // Clean up
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_glob_returns_matches() {
    let tmp = std::env::temp_dir().join("hsab_test_glob");
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    fs::write(tmp.join("file1.rs"), "").unwrap();
    fs::write(tmp.join("file2.rs"), "").unwrap();
    fs::write(tmp.join("other.txt"), "").unwrap();

    let cmd = format!("\"{}/*.rs\" glob", tmp.display());
    let output = eval(&cmd).unwrap();

    // Should contain .rs files
    assert!(output.contains("file1.rs") || output.contains("file2.rs"));
    // Should not contain .txt
    assert!(!output.contains("other.txt"));

    // Clean up
    let _ = fs::remove_dir_all(&tmp);
}
