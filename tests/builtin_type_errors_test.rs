use std::process::Command;

#[test]
fn gt_with_strings_should_exit_with_error() {
    let status = Command::new("cargo")
        .args(["run", "--quiet", "--bin", "test_gt_type_error"])
        .status()
        .expect("Failed to run test binary");

    assert_eq!(
        status.code(),
        Some(1),
        "gt with strings should exit with code 1"
    );
}

#[test]
fn lt_with_strings_should_exit_with_error() {
    let status = Command::new("cargo")
        .args(["run", "--quiet", "--bin", "test_lt_type_error"])
        .status()
        .expect("Failed to run test binary");

    assert_eq!(
        status.code(),
        Some(1),
        "lt with strings should exit with code 1"
    );
}

#[test]
fn concat_with_integers_should_exit_with_error() {
    let status = Command::new("cargo")
        .args(["run", "--quiet", "--bin", "test_concat_type_error"])
        .status()
        .expect("Failed to run test binary");

    assert_eq!(
        status.code(),
        Some(1),
        "concat with integers should exit with code 1"
    );
}
