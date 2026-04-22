use crate::datalog::{parser, Fact, Rule, SourceLocation};
use std::collections::HashMap;

const PRELUDE_FILES: &[(&str, &str)] = &[
    (
        "aliases.datalog",
        include_str!("../../prelude/aliases.datalog"),
    ),
    (
        "computed_attributes.datalog",
        include_str!("../../prelude/computed_attributes.datalog"),
    ),
    (
        "dependency_resolution.datalog",
        include_str!("../../prelude/dependency_resolution.datalog"),
    ),
    (
        "engine_config.datalog",
        include_str!("../../prelude/engine_config.datalog"),
    ),
    (
        "external_data.datalog",
        include_str!("../../prelude/external_data.datalog"),
    ),
    (
        "git_diff.datalog",
        include_str!("../../prelude/git_diff.datalog"),
    ),
    (
        "glob_support.datalog",
        include_str!("../../prelude/glob_support.datalog"),
    ),
    (
        "graph_generation.datalog",
        include_str!("../../prelude/graph_generation.datalog"),
    ),
    (
        "license_validation.datalog",
        include_str!("../../prelude/license_validation.datalog"),
    ),
    (
        "needs_rebuild.datalog",
        include_str!("../../prelude/needs_rebuild.datalog"),
    ),
    (
        "ownership.datalog",
        include_str!("../../prelude/ownership.datalog"),
    ),
    (
        "patterns.datalog",
        include_str!("../../prelude/patterns.datalog"),
    ),
    (
        "query_helpers.datalog",
        include_str!("../../prelude/query_helpers.datalog"),
    ),
    (
        "toolchain_discovery.datalog",
        include_str!("../../prelude/toolchain_discovery.datalog"),
    ),
];

pub fn get_prelude_rules() -> Vec<Rule> {
    let mut all_rules = Vec::new();

    for (name, content) in PRELUDE_FILES {
        match parser::parse_program(content) {
            Ok((_, rules)) => {
                all_rules.extend(rules);
            }
            Err(e) => {
                eprintln!("Failed to parse prelude file {}: {}", name, e);
            }
        }
    }

    all_rules
}

pub fn get_prelude_facts() -> Vec<Fact> {
    let mut all_facts = Vec::new();

    for (name, content) in PRELUDE_FILES {
        match parser::parse_program(content) {
            Ok((facts, _)) => {
                all_facts.extend(facts);
            }
            Err(e) => {
                eprintln!("Failed to parse prelude file {}: {}", name, e);
            }
        }
    }

    all_facts
}

pub fn get_prelude_with_locations() -> (Vec<Fact>, Vec<Rule>, HashMap<String, SourceLocation>) {
    let mut all_facts = Vec::new();
    let mut all_rules = Vec::new();
    let mut all_locations = HashMap::new();

    for (name, content) in PRELUDE_FILES {
        match parser::parse_program_with_file(content, "prelude") {
            Ok((facts, rules, locations)) => {
                all_facts.extend(facts);
                all_rules.extend(rules);
                all_locations.extend(locations);
            }
            Err(e) => {
                eprintln!("Failed to parse prelude file {}: {}", name, e);
            }
        }
    }

    (all_facts, all_rules, all_locations)
}

pub fn get_prelude_content() -> Vec<(String, String)> {
    PRELUDE_FILES
        .iter()
        .map(|(name, content)| (name.to_string(), content.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prelude_parses_correctly() {
        let rules = get_prelude_rules();
        assert!(rules.len() >= 5);

        let needs_rebuild_count = rules
            .iter()
            .filter(|r| r.head.name == "needs_rebuild")
            .count();
        assert_eq!(needs_rebuild_count, 5);

        let transitive_deps_count = rules
            .iter()
            .filter(|r| r.head.name == "transitive_deps")
            .count();
        assert_eq!(transitive_deps_count, 2);

        let toolchain_count = rules
            .iter()
            .filter(|r| r.head.name == "toolchain" || r.head.name == "build_ready")
            .count();
        assert!(toolchain_count >= 3);
    }
}
