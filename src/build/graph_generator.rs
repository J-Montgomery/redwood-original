use super::{BuildKind, BuildPlan};
use crate::datalog::{Engine, TargetLabel, Value};
use std::collections::HashMap;
use std::path::PathBuf;

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

pub struct GraphGenerator;

impl BuildKind for GraphGenerator {
    fn name(&self) -> &str {
        "graph_generator"
    }

    fn plan(&self, target: &TargetLabel, db: &mut Engine) -> Result<BuildPlan, String> {
        let root_target = query_attr(db, target, "root")
            .ok_or_else(|| format!("graph_generator target {} missing 'root' attribute", target))?;

        let query_type =
            query_attr(db, target, "query").unwrap_or_else(|| "transitive_deps".to_string());

        let output_files = query_outputs(db, target);
        if output_files.is_empty() {
            return Err(format!("graph_generator target {} has no outputs", target));
        }
        let output_file = &output_files[0];

        let query_results = match query_type.as_str() {
            "transitive_deps" => db.query("transitive_deps", &[Some(&root_target), None]),
            "all_rdeps" => db.query("all_rdeps", &[Some(&root_target), None]),
            "deps" => db.query("deps", &[Some(&root_target), None]),
            "rdeps" => db.query("rdeps", &[Some(&root_target), None]),
            _ => return Err(format!("Unknown query type: {}", query_type)),
        };

        let mut dot_content = String::from("digraph deps {\n");
        for fact in query_results {
            if fact.args.len() >= 2 {
                if let (Value::String(from), Value::String(to)) = (&fact.args[0], &fact.args[1]) {
                    dot_content.push_str(&format!("  \"{}\" -> \"{}\";\n", from, to));
                }
            }
        }
        dot_content.push_str("}\n");

        let script = format!(
            "cat > {} << 'GRAPHEOF'\n{}\nGRAPHEOF\n",
            output_file.display(),
            dot_content
        );

        Ok(BuildPlan {
            target: target.clone(),
            command: "/bin/sh".to_string(),
            args: vec!["-c".to_string(), script],
            env: HashMap::new(),
            inputs: vec![],
            outputs: vec![output_file.clone()],
            working_dir: PathBuf::from("."),
            no_sandbox: false,
        })
    }
}
