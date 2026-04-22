use crate::build::{BuildPlan, BuildResult};
use crate::datalog::TargetLabel;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

pub struct Sandbox {
    _temp_dir: TempDir,
    sandbox_path: PathBuf,
    original_working_dir: PathBuf,
    outputs: Vec<PathBuf>,
    target: TargetLabel,
}

impl Sandbox {
    pub fn create(plan: &BuildPlan) -> Result<Self, String> {
        let temp_dir = TempDir::new().map_err(|e| {
            format!(
                "Failed to create sandbox for target '{}': {}",
                plan.target, e
            )
        })?;

        let sandbox_path = temp_dir.path().to_path_buf();
        let original_working_dir = plan.working_dir.clone();

        let sandbox = Self {
            _temp_dir: temp_dir,
            sandbox_path,
            original_working_dir,
            outputs: plan.outputs.clone(),
            target: plan.target.clone(),
        };

        sandbox.setup_inputs(&plan.inputs)?;
        sandbox.setup_output_dirs()?;

        Ok(sandbox)
    }

    fn setup_output_dirs(&self) -> Result<(), String> {
        for output in &self.outputs {
            let sandbox_output = self.sandbox_path.join(output);
            if let Some(parent) = sandbox_output.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    format!(
                        "Target '{}': Failed to create output directory in sandbox\n\
                         Path: {}\n\
                         Error: {}",
                        self.target,
                        parent.display(),
                        e
                    )
                })?;
            }
        }
        Ok(())
    }

    fn validate_input_path(&self, input: &PathBuf) -> Result<PathBuf, String> {
        let path_str = input.to_string_lossy();
        if path_str.contains("..") {
            return Err(format!(
                "Target '{}': Security error - Path contains '..' segments\n\
                 Path: {}\n\
                 Workspace root: {}\n\
                 Reason: Relative parent references not allowed\n\
                 Suggestion: Use workspace-relative paths in sources() predicate",
                self.target,
                input.display(),
                self.original_working_dir.display()
            ));
        }

        let input_abs = if input.is_absolute() {
            input.clone()
        } else {
            self.original_working_dir.join(input)
        };

        if !input_abs.exists() {
            return Err(format!(
                "Target '{}': Input file does not exist\n\
                 Path: {}\n\
                 Suggestion: Add this file to the sources() predicate in your BUILD.datalog file",
                self.target,
                input_abs.display()
            ));
        }

        let canonical = input_abs.canonicalize().map_err(|e| {
            format!(
                "Target '{}': Path validation failed\n\
                 Path: {}\n\
                 Workspace root: {}\n\
                 Error: {}",
                self.target,
                input.display(),
                self.original_working_dir.display(),
                e
            )
        })?;

        let canonical_workspace = self.original_working_dir.canonicalize().map_err(|e| {
            format!(
                "Target '{}': Failed to canonicalize workspace root\n\
                 Workspace root: {}\n\
                 Error: {}",
                self.target,
                self.original_working_dir.display(),
                e
            )
        })?;

        if !canonical.starts_with(&canonical_workspace) {
            return Err(format!(
                "Target '{}': Security error - Path traversal detected\n\
                 Input path: {}\n\
                 Resolved path: {}\n\
                 Workspace root: {}\n\
                 Reason: Path escapes workspace after resolution\n\
                 Suggestion: Ensure all files are within the workspace directory",
                self.target,
                input.display(),
                canonical.display(),
                canonical_workspace.display()
            ));
        }

        Ok(input_abs)
    }

    fn setup_inputs(&self, inputs: &[PathBuf]) -> Result<(), String> {
        use std::collections::HashSet;

        let mut seen = HashSet::new();
        let unique_inputs: Vec<&PathBuf> = inputs
            .iter()
            .filter(|input| seen.insert(input.to_string_lossy().to_string()))
            .collect();

        for input in unique_inputs {
            let input_abs = self.validate_input_path(input)?;

            // Mirror absolute paths: /home/user/src/file.c -> sandbox/home/user/src/file.c
            let sandbox_input = if input.is_absolute() {
                let stripped = input.to_string_lossy().trim_start_matches('/').to_string();
                self.sandbox_path.join(stripped)
            } else {
                self.sandbox_path.join(input)
            };

            if sandbox_input.exists() {
                continue;
            }

            if let Some(parent) = sandbox_input.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    format!(
                        "Target '{}': Failed to create sandbox directory\n\
                         Path: {}\n\
                         Error: {}",
                        self.target,
                        parent.display(),
                        e
                    )
                })?;
            }

            symlink(&input_abs, &sandbox_input).map_err(|e| {
                format!(
                    "Target '{}': Failed to create symlink in sandbox\n\
                     Source: {}\n\
                     Target: {}\n\
                     Error: {}",
                    self.target,
                    input_abs.display(),
                    sandbox_input.display(),
                    e
                )
            })?;
        }

        Ok(())
    }

    pub fn execute(&self, plan: &BuildPlan) -> Result<BuildResult, String> {
        let mut cmd = Command::new(&plan.command);
        cmd.args(&plan.args);
        cmd.current_dir(&self.sandbox_path);

        for (key, val) in &plan.env {
            cmd.env(key, val);
        }

        let output = cmd.output().map_err(|e| {
            format!(
                "Target '{}': Failed to execute command in sandbox\n\
                 Command: {} {}\n\
                 Working directory: {}\n\
                 Error: {}",
                self.target,
                plan.command,
                plan.args.join(" "),
                self.sandbox_path.display(),
                e
            )
        })?;

        let success = output.status.success();

        if !success {
            eprintln!("Command failed in sandbox:");
            eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
            eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
        }

        Ok(BuildResult {
            success,
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code(),
        })
    }

    pub fn copy_outputs(&self) -> Result<(), String> {
        for output in &self.outputs {
            let sandbox_output = self.sandbox_path.join(output);

            if !sandbox_output.exists() {
                return Err(format!(
                    "Target '{}': Expected output file not found\n\
                     Expected path: {}\n\
                     Sandbox path: {}\n\
                     Suggestion: Verify the build command creates this file, or check the outputs() predicate in your BUILD.datalog file",
                    self.target,
                    output.display(),
                    sandbox_output.display()
                ));
            }

            let real_output = if output.is_absolute() {
                output.clone()
            } else {
                self.original_working_dir.join(output)
            };

            if let Some(parent) = real_output.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    format!(
                        "Target '{}': Failed to create output directory\n\
                         Path: {}\n\
                         Error: {}",
                        self.target,
                        parent.display(),
                        e
                    )
                })?;
            }

            fs::copy(&sandbox_output, &real_output).map_err(|e| {
                format!(
                    "Target '{}': Failed to copy output from sandbox\n\
                     Source: {}\n\
                     Destination: {}\n\
                     Error: {}",
                    self.target,
                    sandbox_output.display(),
                    real_output.display(),
                    e
                )
            })?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datalog::TargetLabel;
    use std::collections::HashMap;
    use std::env;

    fn create_test_plan(inputs: Vec<PathBuf>, outputs: Vec<PathBuf>) -> BuildPlan {
        BuildPlan {
            target: TargetLabel::new("//test", "target"),
            command: "/bin/true".to_string(),
            args: vec![],
            env: HashMap::new(),
            inputs,
            outputs,
            working_dir: env::current_dir().unwrap(),
            no_sandbox: false,
        }
    }

    #[test]
    fn sandbox_creates_temp_directory() {
        let plan = create_test_plan(vec![], vec![]);
        let sandbox = Sandbox::create(&plan).unwrap();
        assert!(sandbox.sandbox_path.exists());
        assert!(sandbox.sandbox_path.is_dir());
    }

    #[test]
    fn sandbox_symlinks_inputs() {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("input.txt");
        fs::write(&test_file, "test content").unwrap();

        let mut plan = create_test_plan(vec![PathBuf::from("input.txt")], vec![]);
        plan.working_dir = temp_dir.path().to_path_buf();

        let sandbox = Sandbox::create(&plan).unwrap();
        let symlink_path = sandbox.sandbox_path.join("input.txt");

        assert!(symlink_path.exists());
        assert!(symlink_path.is_symlink());

        let content = fs::read_to_string(&symlink_path).unwrap();
        assert_eq!(content, "test content");
    }

    #[test]
    fn sandbox_preserves_directory_structure() {
        let temp_dir = tempfile::tempdir().unwrap();
        let subdir = temp_dir.path().join("src");
        fs::create_dir(&subdir).unwrap();
        let test_file = subdir.join("main.rs");
        fs::write(&test_file, "fn main() {}").unwrap();

        let mut plan = create_test_plan(vec![PathBuf::from("src/main.rs")], vec![]);
        plan.working_dir = temp_dir.path().to_path_buf();

        let sandbox = Sandbox::create(&plan).unwrap();
        let symlink_path = sandbox.sandbox_path.join("src/main.rs");

        assert!(symlink_path.exists());
        assert_eq!(fs::read_to_string(&symlink_path).unwrap(), "fn main() {}");
    }

    #[test]
    fn sandbox_executes_simple_command() {
        let plan = BuildPlan {
            target: TargetLabel::new("//test", "echo"),
            command: "/bin/echo".to_string(),
            args: vec!["hello".to_string()],
            env: HashMap::new(),
            inputs: vec![],
            outputs: vec![],
            working_dir: env::current_dir().unwrap(),
            no_sandbox: false,
        };

        let sandbox = Sandbox::create(&plan).unwrap();
        let result = sandbox.execute(&plan).unwrap();

        assert!(result.success);
        assert!(result.stdout.contains("hello"));
    }

    #[test]
    fn sandbox_copies_outputs_correctly() {
        let temp_dir = tempfile::tempdir().unwrap();
        let output_file = PathBuf::from("output.txt");

        let plan = BuildPlan {
            target: TargetLabel::new("//test", "touch"),
            command: "/bin/sh".to_string(),
            args: vec![
                "-c".to_string(),
                "echo 'output content' > output.txt".to_string(),
            ],
            env: HashMap::new(),
            inputs: vec![],
            outputs: vec![output_file.clone()],
            working_dir: temp_dir.path().to_path_buf(),
            no_sandbox: false,
        };

        let sandbox = Sandbox::create(&plan).unwrap();
        let result = sandbox.execute(&plan).unwrap();
        assert!(result.success);

        sandbox.copy_outputs().unwrap();

        let real_output = temp_dir.path().join(&output_file);
        assert!(real_output.exists());
        let content = fs::read_to_string(&real_output).unwrap();
        assert!(content.contains("output content"));
    }

    #[test]
    fn sandbox_fails_on_missing_input() {
        let plan = create_test_plan(vec![PathBuf::from("nonexistent.txt")], vec![]);
        let result = Sandbox::create(&plan);
        assert!(result.is_err());
        if let Err(msg) = result {
            assert!(msg.contains("Input file does not exist"), "Error message: {}", msg);
            assert!(msg.contains("//test:target"), "Error should include target name");
            assert!(msg.contains("Suggestion:"), "Error should include suggestion");
        }
    }

    #[test]
    fn sandbox_fails_on_missing_output() {
        let temp_dir = tempfile::tempdir().unwrap();
        let output_file = PathBuf::from("missing_output.txt");

        let plan = BuildPlan {
            target: TargetLabel::new("//test", "noop"),
            command: "/bin/true".to_string(),
            args: vec![],
            env: HashMap::new(),
            inputs: vec![],
            outputs: vec![output_file],
            working_dir: temp_dir.path().to_path_buf(),
            no_sandbox: false,
        };

        let sandbox = Sandbox::create(&plan).unwrap();
        let result = sandbox.execute(&plan).unwrap();
        assert!(result.success);

        let copy_result = sandbox.copy_outputs();
        assert!(copy_result.is_err());
        let err_msg = copy_result.unwrap_err();
        assert!(
            err_msg.contains("Expected output file not found"),
            "Error message: {}",
            err_msg
        );
        assert!(
            err_msg.contains("//test:noop"),
            "Error should include target name"
        );
        assert!(
            err_msg.contains("Suggestion:"),
            "Error should include suggestion"
        );
    }

    #[test]
    fn sandbox_cleanup_on_drop() {
        let sandbox_path = {
            let plan = create_test_plan(vec![], vec![]);
            let sandbox = Sandbox::create(&plan).unwrap();
            sandbox.sandbox_path.clone()
        };

        assert!(!sandbox_path.exists());
    }

    #[test]
    fn concurrent_sandbox_creation() {
        use std::sync::Arc;
        use std::thread;

        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("input.txt");
        fs::write(&test_file, "shared input").unwrap();

        let working_dir = Arc::new(temp_dir.path().to_path_buf());

        let handles: Vec<_> = (0..10)
            .map(|i| {
                let dir = Arc::clone(&working_dir);
                thread::spawn(move || {
                    let mut plan = create_test_plan(
                        vec![PathBuf::from("input.txt")],
                        vec![PathBuf::from(format!("output_{}.txt", i))],
                    );
                    plan.working_dir = (*dir).clone();
                    plan.command = "/bin/sh".to_string();
                    plan.args = vec![
                        "-c".to_string(),
                        format!("cat input.txt > output_{}.txt", i),
                    ];

                    let sandbox = Sandbox::create(&plan).unwrap();
                    let result = sandbox.execute(&plan).unwrap();
                    assert!(result.success, "Thread {} build failed", i);
                    sandbox.copy_outputs().unwrap();
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        for i in 0..10 {
            let output = temp_dir.path().join(format!("output_{}.txt", i));
            assert!(output.exists(), "Output {} should exist", i);
            let content = fs::read_to_string(&output).unwrap();
            assert!(
                content.contains("shared input"),
                "Output {} has wrong content",
                i
            );
        }
    }

    #[test]
    fn concurrent_output_directory_creation() {
        use std::sync::Arc;
        use std::thread;

        let temp_dir = tempfile::tempdir().unwrap();
        let working_dir = Arc::new(temp_dir.path().to_path_buf());

        let handles: Vec<_> = (0..10)
            .map(|i| {
                let dir = Arc::clone(&working_dir);
                thread::spawn(move || {
                    let mut plan = create_test_plan(
                        vec![],
                        vec![PathBuf::from(format!("target/outputs/file_{}.txt", i))],
                    );
                    plan.working_dir = (*dir).clone();
                    plan.command = "/bin/sh".to_string();
                    plan.args = vec![
                        "-c".to_string(),
                        format!("echo 'output {}' > target/outputs/file_{}.txt", i, i),
                    ];

                    let sandbox = Sandbox::create(&plan).unwrap();
                    let result = sandbox.execute(&plan).unwrap();
                    assert!(result.success, "Thread {} build failed", i);
                    sandbox.copy_outputs().unwrap();
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        let output_dir = temp_dir.path().join("target/outputs");
        assert!(output_dir.exists());
        assert!(output_dir.is_dir());

        for i in 0..10 {
            let output = output_dir.join(format!("file_{}.txt", i));
            assert!(output.exists(), "Output {} should exist", i);
        }
    }

    #[test]
    fn security_blocks_path_traversal_with_parent_refs() {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("safe.txt");
        fs::write(&test_file, "safe content").unwrap();

        let mut plan = create_test_plan(vec![PathBuf::from("../../../etc/passwd")], vec![]);
        plan.working_dir = temp_dir.path().to_path_buf();

        let result = Sandbox::create(&plan);
        assert!(result.is_err());

        if let Err(err_msg) = result {
            assert!(err_msg.contains("Security error"));
            assert!(err_msg.contains(".."));
            assert!(err_msg.contains("Reason:"));
        }
    }

    #[test]
    fn security_blocks_symlink_escape_outside_workspace() {
        let temp_dir = tempfile::tempdir().unwrap();
        let workspace = temp_dir.path().join("workspace");
        fs::create_dir(&workspace).unwrap();

        let external_target = temp_dir.path().join("external_secret.txt");
        fs::write(&external_target, "secret data").unwrap();

        let malicious_link = workspace.join("innocent_file.txt");
        symlink(&external_target, &malicious_link).unwrap();

        let mut plan = create_test_plan(vec![PathBuf::from("innocent_file.txt")], vec![]);
        plan.working_dir = workspace.clone();

        let result = Sandbox::create(&plan);
        assert!(result.is_err());

        if let Err(err_msg) = result {
            assert!(err_msg.contains("Security error") || err_msg.contains("Path traversal"));
            assert!(err_msg.contains("Workspace root:"));
        }
    }

    #[test]
    fn security_blocks_absolute_path_outside_workspace() {
        let temp_dir = tempfile::tempdir().unwrap();
        let workspace = temp_dir.path().join("workspace");
        fs::create_dir(&workspace).unwrap();

        let test_file = workspace.join("safe.txt");
        fs::write(&test_file, "safe").unwrap();

        let mut plan = create_test_plan(vec![PathBuf::from("/etc/passwd")], vec![]);
        plan.working_dir = workspace.clone();

        let result = Sandbox::create(&plan);
        assert!(result.is_err());
    }

    #[test]
    fn security_allows_valid_relative_paths() {
        let temp_dir = tempfile::tempdir().unwrap();
        let subdir = temp_dir.path().join("src");
        fs::create_dir(&subdir).unwrap();
        let test_file = subdir.join("file.txt");
        fs::write(&test_file, "valid content").unwrap();

        let mut plan = create_test_plan(vec![PathBuf::from("src/file.txt")], vec![]);
        plan.working_dir = temp_dir.path().to_path_buf();

        let sandbox = Sandbox::create(&plan);
        assert!(sandbox.is_ok(), "Valid relative paths should be allowed");
    }

    #[test]
    fn security_allows_valid_absolute_paths_within_workspace() {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_file = temp_dir.path().join("file.txt");
        fs::write(&test_file, "valid content").unwrap();

        let mut plan = create_test_plan(vec![test_file.clone()], vec![]);
        plan.working_dir = temp_dir.path().to_path_buf();

        let sandbox = Sandbox::create(&plan);
        assert!(
            sandbox.is_ok(),
            "Valid absolute paths within workspace should be allowed"
        );
    }

    #[test]
    fn security_allows_symlinks_within_workspace() {
        let temp_dir = tempfile::tempdir().unwrap();
        let target_file = temp_dir.path().join("target.txt");
        fs::write(&target_file, "target content").unwrap();

        let link_file = temp_dir.path().join("link.txt");
        symlink(&target_file, &link_file).unwrap();

        let mut plan = create_test_plan(vec![PathBuf::from("link.txt")], vec![]);
        plan.working_dir = temp_dir.path().to_path_buf();

        let sandbox = Sandbox::create(&plan);
        assert!(
            sandbox.is_ok(),
            "Symlinks within workspace should be allowed"
        );

        if let Ok(sandbox) = sandbox {
            let sandbox_link = sandbox.sandbox_path.join("link.txt");
            assert!(sandbox_link.exists());
            let content = fs::read_to_string(&sandbox_link).unwrap();
            assert_eq!(content, "target content");
        }
    }

    #[test]
    fn security_error_messages_include_context() {
        let temp_dir = tempfile::tempdir().unwrap();

        let mut plan = create_test_plan(vec![PathBuf::from("../../etc/shadow")], vec![]);
        plan.working_dir = temp_dir.path().to_path_buf();

        let result = Sandbox::create(&plan);
        assert!(result.is_err());

        if let Err(err_msg) = result {
            assert!(err_msg.contains("../../etc/shadow"), "Error should mention the problematic path");
            assert!(err_msg.contains("Workspace root:"), "Error should include workspace root");
            assert!(err_msg.contains("Reason:"), "Error should explain why it failed");
            assert!(err_msg.contains("//test:target"), "Error should include target name");
        }
    }

    #[test]
    fn error_messages_include_target_and_suggestions() {
        let temp_dir = tempfile::tempdir().unwrap();

        let mut plan = create_test_plan(vec![PathBuf::from("missing.txt")], vec![]);
        plan.working_dir = temp_dir.path().to_path_buf();

        let result = Sandbox::create(&plan);
        assert!(result.is_err());
        if let Err(msg) = result {
            assert!(msg.contains("Target '//test:target'"), "Missing input error should include target");
            assert!(msg.contains("sources()"), "Missing input error should suggest sources() predicate");
        }

        let output_file = PathBuf::from("output.txt");
        let mut plan = create_test_plan(vec![], vec![output_file]);
        plan.working_dir = temp_dir.path().to_path_buf();
        plan.command = "/bin/true".to_string();

        let sandbox = Sandbox::create(&plan).unwrap();
        sandbox.execute(&plan).unwrap();
        let result = sandbox.copy_outputs();
        assert!(result.is_err());
        if let Err(msg) = result {
            assert!(msg.contains("Target '//test:target'"), "Missing output error should include target");
            assert!(msg.contains("outputs()"), "Missing output error should suggest outputs() predicate");
        }
    }
}
