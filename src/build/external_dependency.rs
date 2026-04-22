use crate::build::{BuildKind, BuildPlan};
use crate::datalog::{Engine, TargetLabel, Value};
use std::path::PathBuf;

pub struct ExternalDependency;

impl Default for ExternalDependency {
    fn default() -> Self {
        Self::new()
    }
}

impl ExternalDependency {
    pub fn new() -> Self {
        ExternalDependency
    }
}

impl BuildKind for ExternalDependency {
    fn name(&self) -> &str {
        "external_dependency"
    }

    fn plan(&self, target: &TargetLabel, db: &mut Engine) -> Result<BuildPlan, String> {
        let target_str = target.to_string();

        // Query outputs - external deps should have their .rlib files
        let outputs_facts = db.query("outputs", &[Some(&target_str)]);
        let outputs: Vec<PathBuf> = outputs_facts
            .iter()
            .filter_map(|f| {
                if f.args.len() >= 2 {
                    if let Value::String(output) = &f.args[1] {
                        return Some(PathBuf::from(output));
                    }
                }
                None
            })
            .collect();

        if outputs.is_empty() {
            return Err(format!(
                "External dependency {} has no outputs. Run 'cargo build --release' to build dependencies.",
                target
            ));
        }

        // No-op build: just verify outputs exist
        // If they don't exist, needs_rebuild will catch it
        Ok(BuildPlan {
            target: target.clone(),
            command: "true".to_string(),
            args: vec![],
            inputs: vec![],
            outputs,
            env: Default::default(),
            working_dir: PathBuf::from("."),
            no_sandbox: true,
        })
    }
}
