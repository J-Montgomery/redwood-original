use crate::datalog::{Fact, Value};
use std::process::Command;

pub struct ToolchainScanner {
    tools: Vec<&'static str>,
}

impl Default for ToolchainScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolchainScanner {
    pub fn new() -> Self {
        Self {
            tools: vec![
                "cargo", "rustc", "gcc", "g++", "clang", "clang++", "go", "python3", "node",
                "echo", "cat", "touch", "cp", "mkdir",
            ],
        }
    }

    pub fn scan(&self) -> Vec<Fact> {
        let mut facts = Vec::new();

        for &tool_name in &self.tools {
            if let Ok(path) = which::which(tool_name) {
                let version = self.get_version(tool_name);
                let path_str = path.to_string_lossy().to_string();

                facts.push(Fact {
                    predicate: "tool_available".to_string(),
                    args: vec![
                        Value::String(tool_name.to_string()),
                        Value::String(path_str),
                        Value::String(version),
                    ],
                });
            }
        }

        facts
    }

    fn get_version(&self, tool_name: &str) -> String {
        let version_flag = match tool_name {
            "node" => "--version",
            _ => "--version",
        };

        Command::new(tool_name)
            .arg(version_flag)
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .or_else(|| {
                Command::new(tool_name)
                    .arg(version_flag)
                    .output()
                    .ok()
                    .and_then(|o| String::from_utf8(o.stderr).ok())
            })
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_finds_tools() {
        let scanner = ToolchainScanner::new();
        let facts = scanner.scan();

        assert!(!facts.is_empty());

        for fact in &facts {
            assert_eq!(fact.predicate, "tool_available");
            assert_eq!(fact.args.len(), 3);
        }
    }
}
