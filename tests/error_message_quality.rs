use redwood::build::{BuildPlan, Executor};
use redwood::datalog::TargetLabel;
use std::collections::HashMap;
use tempfile::tempdir;

#[test]
fn test_build_execution_error_includes_command_and_exit_code() {
    let temp_dir = tempdir().unwrap();
    let target = TargetLabel::parse("//test:failing").unwrap();

    let plan = BuildPlan {
        target: target.clone(),
        command: "/bin/false".to_string(),
        args: vec![],
        env: HashMap::new(),
        inputs: vec![],
        outputs: vec![],
        working_dir: temp_dir.path().to_path_buf(),
        no_sandbox: true,
    };

    let executor = Executor::new();
    let result = executor.execute(&plan);

    // Execution should succeed but the build result should indicate failure
    assert!(result.is_ok());
    let build_result = result.unwrap();
    assert!(!build_result.success);
    assert_eq!(build_result.exit_code, Some(1));
}

#[test]
fn test_sandbox_execution_error_includes_context() {
    let temp_dir = tempdir().unwrap();

    let target = TargetLabel::parse("//test:sandbox_fail").unwrap();
    let plan = BuildPlan {
        target: target.clone(),
        command: "/bin/sh".to_string(),
        args: vec!["-c".to_string(), "exit 42".to_string()],
        env: HashMap::new(),
        inputs: vec![],
        outputs: vec![],
        working_dir: temp_dir.path().to_path_buf(),
        no_sandbox: false,
    };

    let executor = Executor::new();
    let result = executor.execute(&plan);

    assert!(result.is_ok());
    let build_result = result.unwrap();
    assert!(!build_result.success);
    assert_eq!(build_result.exit_code, Some(42));
}
