use redwood::format;
use std::fs;
use tempfile::tempdir;

#[test]
fn format_file_with_messy_spacing() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("BUILD.datalog");

    let messy_content =
        r#"target("//app:cli"):-kind("//app:cli",rust_binary),sources("//app:cli","main.rs")."#;
    fs::write(&file_path, messy_content).unwrap();

    let formatted = format::format_file(&file_path).unwrap();

    assert!(formatted.contains("target(\"//app:cli\") :-"));
    assert!(formatted.contains("    kind(\"//app:cli\", rust_binary),"));
    assert!(formatted.contains("    sources(\"//app:cli\", \"main.rs\")."));
}

#[test]
fn format_file_detects_syntax_errors() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("BUILD.datalog");

    let invalid_content = r#"target("//app:cli":-broken"#;
    fs::write(&file_path, invalid_content).unwrap();

    let result = format::format_file(&file_path);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Syntax error"));
}

#[test]
fn find_build_files_in_directory() {
    let dir = tempdir().unwrap();

    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("BUILD.datalog"), "target(\"//app:cli\").").unwrap();
    fs::write(
        dir.path().join("src/BUILD.datalog"),
        "target(\"//lib:foo\").",
    )
    .unwrap();

    let files = format::find_build_files(dir.path()).unwrap();
    assert_eq!(files.len(), 2);
}

#[test]
fn format_preserves_atoms() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("BUILD.datalog");

    let content = r#"kind("//app:cli",rust_binary)."#;
    fs::write(&file_path, content).unwrap();

    let formatted = format::format_file(&file_path).unwrap();
    assert!(formatted.contains("rust_binary"));
    assert!(!formatted.contains("\"rust_binary\""));
}

#[test]
fn format_handles_negation() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("BUILD.datalog");

    let content = r#"missing(X):-target(X),not(file_exists(X))."#;
    fs::write(&file_path, content).unwrap();

    let formatted = format::format_file(&file_path).unwrap();
    assert!(formatted.contains("not(file_exists(X))"));
}

#[test]
fn format_handles_inequality() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("BUILD.datalog");

    let content = r#"changed(X):-hash(X,H1),old_hash(X,H2),H1!=H2."#;
    fs::write(&file_path, content).unwrap();

    let formatted = format::format_file(&file_path).unwrap();
    assert!(formatted.contains("H1 != H2"));
}

#[test]
fn format_preserves_comments() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("BUILD.datalog");

    let content = r#"# This is a comment
target("//app:cli").
# Another comment
kind("//app:cli",rust_binary)."#;
    fs::write(&file_path, content).unwrap();

    let formatted = format::format_file(&file_path).unwrap();
    assert!(formatted.contains("# This is a comment"));
    assert!(formatted.contains("# Another comment"));
}
