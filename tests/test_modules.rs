//! Integration tests for modules operations

#[path = "common/mod.rs"]
mod common;
#[allow(unused_imports)]
use common::{eval, eval_exit_code, Evaluator, lex, parse};

#[test]
fn test_import_creates_namespaced_definitions() {
    use std::io::Write;
    // Create a temp module file with .hsab extension
    let temp_dir = tempfile::tempdir().unwrap();
    let module_path = temp_dir.path().join("mymodule.hsab");
    let mut file = std::fs::File::create(&module_path).unwrap();
    writeln!(file, "#[dup .bak suffix] :mybackup").unwrap();
    drop(file);

    // Import and call the namespaced function (namespace = "mymodule")
    let code = format!(r#""{}" .import file.txt mymodule::mybackup"#, module_path.display());
    let output = eval(&code).unwrap();
    assert!(output.contains("file.txt.bak"), "Expected namespaced function to work: {}", output);
}

#[test]
fn test_import_with_alias() {
    use std::io::Write;
    let temp_dir = tempfile::tempdir().unwrap();
    let module_path = temp_dir.path().join("mymodule.hsab");
    let mut file = std::fs::File::create(&module_path).unwrap();
    writeln!(file, "#[dup .bak suffix] :mybackup").unwrap();
    drop(file);

    // Import with explicit alias
    let code = format!(r#""{}" utils .import file.txt utils::mybackup"#, module_path.display());
    let output = eval(&code).unwrap();
    assert!(output.contains("file.txt.bak"), "Expected aliased function to work: {}", output);
}

#[test]
fn test_import_private_definitions_not_exported() {
    use std::io::Write;
    let temp_dir = tempfile::tempdir().unwrap();
    let module_path = temp_dir.path().join("mymodule.hsab");
    let mut file = std::fs::File::create(&module_path).unwrap();
    writeln!(file, "#[helper] :_private").unwrap();
    writeln!(file, "#[_private echo] :public").unwrap();
    drop(file);

    // Import - private definitions should not be accessible
    let code = format!(r#""{}" .import mymodule::_private"#, module_path.display());
    let output = eval(&code).unwrap();
    // _private should be treated as literal (not found as definition)
    assert!(output.contains("mymodule::_private"), "Private definitions should not be exported: {}", output);
}

#[test]
fn test_import_skips_already_loaded() {
    use std::io::Write;
    let temp_dir = tempfile::tempdir().unwrap();
    let module_path = temp_dir.path().join("mymodule.hsab");
    let mut file = std::fs::File::create(&module_path).unwrap();
    // This module pushes "loaded" to stack when imported
    writeln!(file, "loaded").unwrap();
    drop(file);

    // Import twice - should only add "loaded" once
    // depth should show 1 because only one import actually executed
    let code = format!(r#""{0}" .import "{0}" .import depth"#, module_path.display());
    let output = eval(&code).unwrap();
    // Output will be "loaded\n1" - the literal and the depth
    assert!(output.contains("1"), "Module should only be loaded once, depth should be 1: {}", output);
    // Make sure there's only ONE "loaded" in the output
    assert_eq!(output.matches("loaded").count(), 1, "Module should only be loaded once: {}", output);
}

