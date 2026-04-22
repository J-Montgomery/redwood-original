use super::{BuildKind, BuildPlan};
use crate::datalog::Engine;
use crate::datalog::{Fact, TargetLabel, Value};
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

const RESERVED_ATTRS: &[&str] = &["tool", "output_dir", "no_sandbox"];

fn validate_tool_name(tool: &str) -> Result<(), String> {
    if tool.is_empty() {
        return Err("Tool name cannot be empty".to_string());
    }

    if tool.contains('\0') {
        return Err(format!("Tool name contains null byte: {}", tool));
    }

    let dangerous_patterns = [";", "|", "&", "$", "`", "\\", "\n", "\r"];
    for pattern in &dangerous_patterns {
        if tool.contains(pattern) {
            return Err(format!(
                "Tool name '{}' contains dangerous shell metacharacter: {}",
                tool, pattern
            ));
        }
    }

    for ch in tool.chars() {
        if !ch.is_alphanumeric() && ch != '-' && ch != '_' && ch != '/' && ch != '.' && ch != '+' {
            return Err(format!(
                "Tool name '{}' contains invalid character '{}'. Only alphanumeric, -, _, /, ., + are allowed",
                tool, ch
            ));
        }
    }

    Ok(())
}

fn validate_argument(arg: &str) -> Result<(), String> {
    if arg.contains('\0') {
        return Err(format!("Argument contains null byte: {}", arg));
    }

    let dangerous_patterns = [";", "|", "&", "$", "`", "$(", "${"];
    for pattern in &dangerous_patterns {
        if arg.contains(pattern) {
            return Err(format!(
                "Argument '{}' contains dangerous shell metacharacter or pattern: {}",
                arg, pattern
            ));
        }
    }

    Ok(())
}

fn query_sources(db: &mut Engine, target: &TargetLabel) -> Vec<PathBuf> {
    let sources_facts = db.query("sources", &[]);
    let target_str = target.to_string();

    sources_facts
        .iter()
        .filter_map(|fact| {
            if fact.args.len() >= 2 {
                if let (Value::String(label), Value::String(path)) = (&fact.args[0], &fact.args[1])
                {
                    if label == &target_str {
                        return Some(PathBuf::from(path));
                    }
                }
            }
            None
        })
        .collect()
}

fn query_outputs(db: &mut Engine, target: &TargetLabel) -> Vec<PathBuf> {
    let outputs_facts = db.query("outputs", &[]);
    let target_str = target.to_string();

    outputs_facts
        .iter()
        .filter_map(|fact| {
            if fact.args.len() >= 2 {
                if let (Value::String(label), Value::String(path)) = (&fact.args[0], &fact.args[1])
                {
                    if label == &target_str {
                        return Some(PathBuf::from(path));
                    }
                }
            }
            None
        })
        .collect()
}

fn query_attr(db: &mut Engine, target: &TargetLabel, key: &str) -> Option<String> {
    let attr_facts = db.query("attr", &[]);
    let target_str = target.to_string();

    attr_facts.iter().find_map(|fact| {
        if fact.args.len() >= 3 {
            if let (Value::String(t), Value::String(k), Value::String(v)) =
                (&fact.args[0], &fact.args[1], &fact.args[2])
            {
                if t == &target_str && k == key {
                    return Some(v.clone());
                }
            }
        }
        None
    })
}

fn query_arg_attrs(db: &mut Engine, target: &TargetLabel, tool: &str) -> Vec<(String, String)> {
    let attr_facts = db.query("attr", &[]);
    let target_str = target.to_string();
    let target_tool_combo = format!("{}:{}", target_str, tool);

    attr_facts
        .iter()
        .filter_map(|fact| {
            if fact.args.len() >= 3 {
                if let (Value::String(t), Value::String(k), Value::String(v)) =
                    (&fact.args[0], &fact.args[1], &fact.args[2])
                {
                    if !RESERVED_ATTRS.contains(&k.as_str())
                        && (t == &target_str || t == tool || t == &target_tool_combo)
                    {
                        return Some((k.clone(), v.clone()));
                    }
                }
            }
            None
        })
        .collect()
}

fn expand_template(
    template: &str,
    sources: &[PathBuf],
    output: &Path,
    target: &TargetLabel,
) -> String {
    let mut result = template.to_string();

    result = result.replace("{output}", &output.to_string_lossy());
    result = result.replace("{target}", &target.to_string());

    if result.contains("{sources}") {
        let sources_str = sources
            .iter()
            .map(|s| s.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(" ");
        result = result.replace("{sources}", &sources_str);
    }

    result
}

fn build_args(
    arg_attrs: Vec<(String, String)>,
    sources: &[PathBuf],
    output: &Path,
    target: &TargetLabel,
) -> Vec<String> {
    let mut args = Vec::new();
    let mut sources_included = false;

    // Sort by numeric key for positional args (0, 1, 2, ...), others keep original order
    let mut sorted_attrs = arg_attrs;
    sorted_attrs.sort_by(|a, b| match (a.0.parse::<i32>(), b.0.parse::<i32>()) {
        (Ok(a_num), Ok(b_num)) => a_num.cmp(&b_num),
        (Ok(_), Err(_)) => std::cmp::Ordering::Less,
        (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
        (Err(_), Err(_)) => std::cmp::Ordering::Equal,
    });

    for (key, value) in sorted_attrs {
        let expanded = expand_template(&value, sources, output, target);

        // Handle template expansions
        if value == "{sources}" || expanded.contains("{sources}") {
            // This is a sources template - add sources here
            if key == "-c" {
                args.push(key.clone());
            }
            for source in sources {
                args.push(source.to_string_lossy().to_string());
            }
            sources_included = true;
        } else if value == "{output}" || expanded.contains("{output}") {
            // This is an output template
            if key.parse::<i32>().is_err() {
                args.push(key.clone());
            }
            args.push(output.to_string_lossy().to_string());
        } else if key.parse::<i32>().is_ok() {
            // Numeric key - only add the value (positional argument)
            args.push(expanded);
        } else {
            // Regular flag - split on space if it contains one
            // (for flags like "-isystem /path" that should be two args)
            if key.contains(' ') {
                for part in key.split_whitespace() {
                    args.push(part.to_string());
                }
            } else {
                args.push(key.clone());
            }
            if !expanded.is_empty() {
                args.push(expanded);
            }
        }
    }

    // If sources weren't explicitly added via template, add them at the end
    // But skip sources that already appear in args (for rustc single-file builds)
    if !sources_included {
        for source in sources {
            let source_str = source.to_string_lossy().to_string();
            if !args.contains(&source_str) {
                args.push(source_str);
            }
        }
    }

    args
}

pub struct SystemTool;

impl Default for SystemTool {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemTool {
    pub fn new() -> Self {
        Self
    }
}

impl BuildKind for SystemTool {
    fn name(&self) -> &str {
        "system_tool"
    }

    fn plan(&self, target: &TargetLabel, db: &mut Engine) -> Result<BuildPlan, String> {
        let tool = query_attr(db, target, "tool").ok_or_else(|| {
            format!(
                "system_tool requires 'tool' attribute for target {}",
                target
            )
        })?;

        validate_tool_name(&tool)?;

        db.insert_facts(vec![Fact {
            predicate: "requires_tool".to_string(),
            args: vec![
                Value::String(target.to_string()),
                Value::String(tool.clone()),
            ],
        }]);

        let sources = query_sources(db, target);

        let constraint_failed_facts = db.query("constraint_failed", &[]);
        let target_str = target.to_string();
        for fact in constraint_failed_facts.iter() {
            if fact.args.len() >= 2 {
                if let (Value::String(t), Value::String(message)) = (&fact.args[0], &fact.args[1]) {
                    if t == &target_str {
                        return Err(format!("Target {} {}", target, message));
                    }
                }
            }
        }

        // Check if outputs are declared, otherwise generate default
        let declared_outputs = query_outputs(db, target);
        let output_path = if !declared_outputs.is_empty() {
            declared_outputs[0].clone()
        } else {
            let output_dir =
                query_attr(db, target, "output_dir").unwrap_or_else(|| format!("target/{}", tool));

            let output_name = target.target.replace(':', "_");
            let dir = PathBuf::from(&output_dir);
            std::fs::create_dir_all(&dir)
                .map_err(|e| format!("Failed to create output directory {}: {}", output_dir, e))?;
            dir.join(output_name)
        };

        // Ensure output directory exists
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create output directory: {}", e))?;
        }

        let arg_attrs = query_arg_attrs(db, target, &tool);
        let args = build_args(arg_attrs, &sources[..], &output_path, target);

        for arg in &args {
            validate_argument(arg)?;
        }

        let tool_facts = db.query("toolchain", &[]);
        let target_str = target.to_string();

        let tool_path = tool_facts
            .iter()
            .find_map(|fact| {
                if fact.args.len() >= 3 {
                    if let (Value::String(t), Value::String(tool_name), Value::String(path)) =
                        (&fact.args[0], &fact.args[1], &fact.args[2])
                    {
                        if t == &target_str && tool_name == &tool {
                            return Some(path.clone());
                        }
                    }
                }
                None
            })
            .ok_or_else(|| format!("{} not found for target {}", tool, target))?;

        validate_argument(&tool_path)?;

        let no_sandbox = query_attr(db, target, "no_sandbox")
            .map(|v| v == "true")
            .unwrap_or(false);

        Ok(BuildPlan {
            target: target.clone(),
            command: tool_path,
            args,
            env: HashMap::new(),
            inputs: sources,
            outputs: vec![output_path],
            working_dir: env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            no_sandbox,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_tool_name_valid() {
        assert!(validate_tool_name("gcc").is_ok());
        assert!(validate_tool_name("rustc").is_ok());
        assert!(validate_tool_name("clang++").is_ok());
        assert!(validate_tool_name("/usr/bin/gcc").is_ok());
        assert!(validate_tool_name("gcc-13").is_ok());
        assert!(validate_tool_name("gcc_wrapper").is_ok());
        assert!(validate_tool_name("tool.sh").is_ok());
    }

    #[test]
    fn test_validate_tool_name_with_semicolon() {
        let result = validate_tool_name("gcc; rm -rf /");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("dangerous shell metacharacter"));
    }

    #[test]
    fn test_validate_tool_name_with_pipe() {
        let result = validate_tool_name("gcc | cat");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("dangerous shell metacharacter"));
    }

    #[test]
    fn test_validate_tool_name_with_ampersand() {
        let result = validate_tool_name("gcc &");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("dangerous shell metacharacter"));
    }

    #[test]
    fn test_validate_tool_name_with_backtick() {
        let result = validate_tool_name("gcc`whoami`");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("dangerous shell metacharacter"));
    }

    #[test]
    fn test_validate_tool_name_with_null_byte() {
        let result = validate_tool_name("gcc\0");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("null byte"));
    }

    #[test]
    fn test_validate_tool_name_empty() {
        let result = validate_tool_name("");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot be empty"));
    }

    #[test]
    fn test_validate_tool_name_with_invalid_chars() {
        let result = validate_tool_name("gcc<test");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid character"));
    }

    #[test]
    fn test_validate_argument_valid() {
        assert!(validate_argument("-o").is_ok());
        assert!(validate_argument("output.o").is_ok());
        assert!(validate_argument("/path/to/file.c").is_ok());
        assert!(validate_argument("-I/usr/include").is_ok());
        assert!(validate_argument("--flag=value").is_ok());
    }

    #[test]
    fn test_validate_argument_with_command_substitution() {
        let result = validate_argument("$(rm -rf /)");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("dangerous"));
    }

    #[test]
    fn test_validate_argument_with_variable_expansion() {
        let result = validate_argument("${HOME}");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("dangerous"));
    }

    #[test]
    fn test_validate_argument_with_pipe() {
        let result = validate_argument("foo | rm -rf /");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("dangerous"));
    }

    #[test]
    fn test_validate_argument_with_semicolon() {
        let result = validate_argument("foo; rm -rf /");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("dangerous"));
    }

    #[test]
    fn test_validate_argument_with_backtick() {
        let result = validate_argument("`whoami`");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("dangerous"));
    }

    #[test]
    fn test_validate_argument_with_null_byte() {
        let result = validate_argument("test\0");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("null byte"));
    }
}
