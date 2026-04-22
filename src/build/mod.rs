pub mod external_dependency;
pub mod graph_generator;
pub mod system_tool;

use crate::datalog::Engine;
use crate::datalog::TargetLabel;
use std::collections::HashMap;
use std::path::PathBuf;

fn validate_env_variables(env: &HashMap<String, String>) -> Result<(), String> {
    for (key, value) in env {
        if key.contains('\0') {
            return Err(format!("Environment key contains null byte: {}", key));
        }
        if value.contains('\0') {
            return Err(format!("Environment value contains null byte: {}", value));
        }

        let dangerous_patterns = [";", "|", "&", "$", "`", "$(", "${"];
        for pattern in &dangerous_patterns {
            if value.contains(pattern) {
                return Err(format!(
                    "Environment value for '{}' contains dangerous shell metacharacter or pattern: {}",
                    key, pattern
                ));
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct BuildPlan {
    pub target: TargetLabel,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub inputs: Vec<PathBuf>,
    pub outputs: Vec<PathBuf>,
    pub working_dir: PathBuf,
    pub no_sandbox: bool,
}

pub trait BuildKind {
    fn name(&self) -> &str;
    fn plan(&self, target: &TargetLabel, db: &mut Engine) -> Result<BuildPlan, String>;
}

pub struct Executor;

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

impl Executor {
    pub fn new() -> Self {
        Self
    }

    pub fn execute(&self, plan: &BuildPlan) -> Result<BuildResult, String> {
        println!("Building {} ...", plan.target);

        validate_env_variables(&plan.env)?;

        if plan.no_sandbox {
            // Execute without sandbox
            let output = std::process::Command::new(&plan.command)
                .args(&plan.args)
                .current_dir(&plan.working_dir)
                .envs(&plan.env)
                .output()
                .map_err(|e| {
                    format!(
                        "Target '{}': Failed to execute command\n\
                         Command: {} {}\n\
                         Working directory: {}\n\
                         Error: {}",
                        plan.target,
                        plan.command,
                        plan.args.join(" "),
                        plan.working_dir.display(),
                        e
                    )
                })?;

            let result = BuildResult {
                success: output.status.success(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code(),
            };

            if result.success {
                println!("Built {} successfully", plan.target);
            } else {
                let exit_info = match result.exit_code {
                    Some(code) => format!("Exit code: {}", code),
                    None => "Process terminated by signal".to_string(),
                };

                eprintln!(
                    "Target '{}': Build execution failed\n\
                     Command: {} {}\n\
                     Working directory: {}\n\
                     {}\n\
                     \n\
                     stdout:\n{}\n\
                     stderr:\n{}",
                    plan.target,
                    plan.command,
                    plan.args.join(" "),
                    plan.working_dir.display(),
                    exit_info,
                    result.stdout,
                    result.stderr
                );
            }

            Ok(result)
        } else {
            use crate::sandbox::Sandbox;

            // Create sandbox and execute
            let sandbox = Sandbox::create(plan)?;
            let result = sandbox.execute(plan)?;

            if result.success {
                sandbox.copy_outputs()?;
                println!("Built {} successfully", plan.target);
            } else {
                let exit_info = match result.exit_code {
                    Some(code) => format!("Exit code: {}", code),
                    None => "Process terminated by signal".to_string(),
                };

                eprintln!(
                    "Target '{}': Build execution failed (sandboxed)\n\
                     Command: {} {}\n\
                     Working directory: {}\n\
                     {}\n\
                     \n\
                     stdout:\n{}\n\
                     stderr:\n{}",
                    plan.target,
                    plan.command,
                    plan.args.join(" "),
                    plan.working_dir.display(),
                    exit_info,
                    result.stdout,
                    result.stderr
                );
            }

            Ok(result)
        }
    }
}

pub struct BuildResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

pub struct BuildKindRegistry {
    kinds: HashMap<String, Box<dyn BuildKind>>,
}

impl Default for BuildKindRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildKindRegistry {
    pub fn new() -> Self {
        Self {
            kinds: HashMap::new(),
        }
    }

    pub fn register(&mut self, kind: Box<dyn BuildKind>) {
        self.kinds.insert(kind.name().to_string(), kind);
    }

    pub fn get(&self, name: &str) -> Option<&dyn BuildKind> {
        self.kinds.get(name).map(|b| b.as_ref())
    }
}
