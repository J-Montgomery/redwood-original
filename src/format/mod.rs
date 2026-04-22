use crate::datalog::{parser, Fact, Predicate, Rule, Term, Value};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub fn format_file(path: &Path) -> Result<String, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    match parser::parse_program(&content) {
        Ok((facts, rules)) => {
            let items = extract_items_with_comments(&content, facts.len(), rules.len());
            Ok(format_with_items(&facts, &rules, &items))
        }
        Err(e) => Err(format!("Syntax error in {}: {}", path.display(), e)),
    }
}

#[derive(Debug)]
enum Item {
    Comment(String),
    FactIndex(usize),
    RuleIndex(usize),
}

fn extract_items_with_comments(content: &str, fact_count: usize, rule_count: usize) -> Vec<Item> {
    let mut items = Vec::new();
    let mut fact_idx = 0;
    let mut rule_idx = 0;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with('#') {
            items.push(Item::Comment(trimmed.to_string()));
        } else if trimmed.contains(":-") {
            if rule_idx < rule_count {
                items.push(Item::RuleIndex(rule_idx));
                rule_idx += 1;
            }
        } else if trimmed.ends_with('.') && !trimmed.contains(":-") && fact_idx < fact_count {
            items.push(Item::FactIndex(fact_idx));
            fact_idx += 1;
        }
    }

    while fact_idx < fact_count {
        items.push(Item::FactIndex(fact_idx));
        fact_idx += 1;
    }
    while rule_idx < rule_count {
        items.push(Item::RuleIndex(rule_idx));
        rule_idx += 1;
    }

    items
}

pub fn format_program(facts: &[Fact], rules: &[Rule]) -> String {
    let items: Vec<Item> = facts
        .iter()
        .enumerate()
        .map(|(i, _)| Item::FactIndex(i))
        .chain(rules.iter().enumerate().map(|(i, _)| Item::RuleIndex(i)))
        .collect();
    format_with_items(facts, rules, &items)
}

fn format_with_items(facts: &[Fact], rules: &[Rule], items: &[Item]) -> String {
    let mut output = String::new();
    let mut last_was_rule = false;

    for item in items {
        match item {
            Item::Comment(comment) => {
                output.push_str(comment);
                output.push('\n');
                last_was_rule = false;
            }
            Item::FactIndex(idx) => {
                if let Some(fact) = facts.get(*idx) {
                    output.push_str(&format_fact(fact));
                    output.push('\n');
                    last_was_rule = false;
                }
            }
            Item::RuleIndex(idx) => {
                if last_was_rule {
                    output.push('\n');
                }
                if let Some(rule) = rules.get(*idx) {
                    output.push_str(&format_rule(rule));
                    output.push('\n');
                    last_was_rule = true;
                }
            }
        }
    }

    output
}

fn format_fact(fact: &Fact) -> String {
    let args = fact
        .args
        .iter()
        .map(format_value)
        .collect::<Vec<_>>()
        .join(", ");

    format!("{}({}).", fact.predicate, args)
}

fn format_rule(rule: &Rule) -> String {
    let mut output = String::new();

    output.push_str(&format_predicate(&rule.head));
    output.push_str(" :-\n");

    for (i, pred) in rule.body.iter().enumerate() {
        output.push_str("    ");
        output.push_str(&format_predicate(pred));

        if i < rule.body.len() - 1 {
            output.push(',');
        } else {
            output.push('.');
        }

        output.push('\n');
    }

    output.trim_end().to_string()
}

fn format_predicate(pred: &Predicate) -> String {
    if pred.name == "!=" && pred.args.len() == 2 {
        return format!(
            "{} != {}",
            format_term(&pred.args[0]),
            format_term(&pred.args[1])
        );
    }

    if pred.name.starts_with("not:") {
        let inner_name = pred.name.strip_prefix("not:").unwrap();
        let args = pred
            .args
            .iter()
            .map(format_term)
            .collect::<Vec<_>>()
            .join(", ");
        return format!("not({}({}))", inner_name, args);
    }

    let args = pred
        .args
        .iter()
        .map(format_term)
        .collect::<Vec<_>>()
        .join(", ");

    format!("{}({})", pred.name, args)
}

fn format_term(term: &Term) -> String {
    match term {
        Term::Variable(v) => v.clone(),
        Term::Constant(val) => format_value(val),
    }
}

fn format_value(val: &Value) -> String {
    match val {
        Value::String(s) => {
            if is_atom(s) {
                s.clone()
            } else {
                format!("\"{}\"", s)
            }
        }
        Value::Integer(i) => i.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Label(l) => format!("\"{}\"", l),
        Value::Path(p) => format!("\"{}\"", p.display()),
    }
}

fn is_atom(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    let first_char = s.chars().next().unwrap();
    if !first_char.is_lowercase() && first_char != '_' {
        return false;
    }

    s.chars().all(|c| c.is_alphanumeric() || c == '_')
}

pub fn find_build_files(path: &Path) -> Result<Vec<String>, String> {
    if path.is_file() {
        return Ok(vec![path.to_string_lossy().to_string()]);
    }

    if !path.is_dir() {
        return Err(format!("{} is not a file or directory", path.display()));
    }

    let mut build_files = Vec::new();

    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            if let Some(filename) = entry.path().file_name() {
                if filename == "BUILD.datalog" {
                    build_files.push(entry.path().to_string_lossy().to_string());
                }
            }
        }
    }

    Ok(build_files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_simple_fact() {
        let content = r#"target("//app:cli")."#;
        let (facts, rules) = parser::parse_program(content).unwrap();
        let formatted = format_program(&facts, &rules);
        assert_eq!(formatted, "target(\"//app:cli\").\n");
    }

    #[test]
    fn format_simple_rule() {
        let content = r#"target(X):-kind(X,rust_binary)."#;
        let (facts, rules) = parser::parse_program(content).unwrap();
        let formatted = format_program(&facts, &rules);
        assert_eq!(formatted, "target(X) :-\n    kind(X, rust_binary).\n");
    }

    #[test]
    fn format_multiline_rule() {
        let content = r#"target(X):-kind(X,rust_binary),sources(X,F),file_exists(F)."#;
        let (facts, rules) = parser::parse_program(content).unwrap();
        let formatted = format_program(&facts, &rules);
        assert_eq!(
            formatted,
            "target(X) :-\n    kind(X, rust_binary),\n    sources(X, F),\n    file_exists(F).\n"
        );
    }

    #[test]
    fn format_negation() {
        let content = r#"missing(X):-target(X),not(file_exists(X))."#;
        let (facts, rules) = parser::parse_program(content).unwrap();
        let formatted = format_program(&facts, &rules);
        assert_eq!(
            formatted,
            "missing(X) :-\n    target(X),\n    not(file_exists(X)).\n"
        );
    }

    #[test]
    fn format_inequality() {
        let content = r#"changed(X):-old_hash(X,O),new_hash(X,N),N!=O."#;
        let (facts, rules) = parser::parse_program(content).unwrap();
        let formatted = format_program(&facts, &rules);
        assert_eq!(
            formatted,
            "changed(X) :-\n    old_hash(X, O),\n    new_hash(X, N),\n    N != O.\n"
        );
    }

    #[test]
    fn format_mixed_facts_and_rules() {
        let content = r#"
            target("//app:cli").
            kind("//app:cli",rust_binary).
            target(X):-kind(X,rust_binary).
        "#;
        let (facts, rules) = parser::parse_program(content).unwrap();
        let formatted = format_program(&facts, &rules);
        assert!(formatted.contains("target(\"//app:cli\").\n"));
        assert!(formatted.contains("kind(\"//app:cli\", rust_binary).\n"));
        assert!(formatted.contains("target(X) :-\n    kind(X, rust_binary).\n"));
    }
}
