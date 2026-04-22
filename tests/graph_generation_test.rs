use redwood::build::graph_generator::GraphGenerator;
use redwood::build::BuildKind;
use redwood::datalog::{parser, Engine, TargetLabel};

#[test]
fn graph_generator_creates_dot_file() {
    let program = r#"
        target("//test:a").
        target("//test:b").
        target("//test:c").

        deps("//test:b", "//test:a").
        deps("//test:c", "//test:b").

        graph_generator("//graphs:test_deps").
        attr("//graphs:test_deps", "root", "//test:c").
        attr("//graphs:test_deps", "query", "transitive_deps").
        outputs("//graphs:test_deps", "test_graph.dot").
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    // Load prelude for transitive_deps
    let (prelude_facts, prelude_rules, _) = redwood::runtime::prelude::get_prelude_with_locations();
    db.insert_facts(prelude_facts);
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let target = TargetLabel::parse("//graphs:test_deps").unwrap();
    let generator = GraphGenerator;

    let plan = generator.plan(&target, &mut db).unwrap();

    assert_eq!(plan.command, "/bin/sh");
    assert_eq!(plan.outputs.len(), 1);
    assert_eq!(plan.outputs[0].to_string_lossy(), "test_graph.dot");

    // Execute the plan
    std::fs::remove_file("test_graph.dot").ok();
    let output = std::process::Command::new(&plan.command)
        .args(&plan.args)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify the DOT file was created
    assert!(std::path::Path::new("test_graph.dot").exists());

    let content = std::fs::read_to_string("test_graph.dot").unwrap();
    assert!(content.contains("digraph deps"));
    assert!(content.contains("\"//test:c\" -> \"//test:b\""));
    assert!(content.contains("\"//test:c\" -> \"//test:a\""));
    // Note: transitive_deps("//test:c", D) returns only edges FROM //test:c,
    // not all edges in the transitive closure. So "//test:b" -> "//test:a" won't be included.

    // Clean up
    std::fs::remove_file("test_graph.dot").unwrap();
}

#[test]
fn graph_generator_with_prelude_integration() {
    // Test that graph generation works with the prelude's graph() helper
    let program = r#"
        target("//lib:core").
        target("//lib:utils").
        target("//app:main").

        deps("//lib:utils", "//lib:core").
        deps("//app:main", "//lib:utils").
        deps("//app:main", "//lib:core").

        # Use the prelude helper to define a graph target
        graph_generator("//graphs:app_deps").
        attr("//graphs:app_deps", "root", "//app:main").
        attr("//graphs:app_deps", "query", "transitive_deps").
        outputs("//graphs:app_deps", "app_deps.dot").
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    // Load prelude
    let (prelude_facts, prelude_rules, _) = redwood::runtime::prelude::get_prelude_with_locations();
    db.insert_facts(prelude_facts);
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    // Query the graph predicate to verify prelude integration
    let graph_query = db.query("graph", &[]);
    assert!(
        !graph_query.is_empty(),
        "graph() predicate should derive facts from graph_generator targets"
    );

    // Verify the graph target is recognized
    let graph_targets = db.query("graph_target", &[]);
    assert_eq!(graph_targets.len(), 1);

    // Generate the graph
    let target = TargetLabel::parse("//graphs:app_deps").unwrap();
    let generator = GraphGenerator;
    let plan = generator.plan(&target, &mut db).unwrap();

    // Execute and verify
    std::fs::remove_file("app_deps.dot").ok();
    let output = std::process::Command::new(&plan.command)
        .args(&plan.args)
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(std::path::Path::new("app_deps.dot").exists());

    let content = std::fs::read_to_string("app_deps.dot").unwrap();
    assert!(content.contains("digraph deps"));
    assert!(content.contains("\"//app:main\" -> \"//lib:utils\""));
    assert!(content.contains("\"//app:main\" -> \"//lib:core\""));

    // Clean up
    std::fs::remove_file("app_deps.dot").unwrap();
}

#[test]
fn graph_generator_with_different_query_types() {
    // Test that different query types work
    let program = r#"
        target("//a").
        target("//b").
        deps("//b", "//a").
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let (prelude_facts, prelude_rules, _) = redwood::runtime::prelude::get_prelude_with_locations();
    db.insert_facts(prelude_facts);
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    // Test deps query type
    let (test_facts, _) = parser::parse_program(
        r#"
        graph_generator("//graphs:test").
        attr("//graphs:test", "root", "//b").
        attr("//graphs:test", "query", "deps").
        outputs("//graphs:test", "deps_graph.dot").
    "#,
    )
    .unwrap();

    db.insert_facts(test_facts);

    let target = TargetLabel::parse("//graphs:test").unwrap();
    let generator = GraphGenerator;
    let plan = generator.plan(&target, &mut db).unwrap();

    std::fs::remove_file("deps_graph.dot").ok();
    let output = std::process::Command::new(&plan.command)
        .args(&plan.args)
        .output()
        .unwrap();

    assert!(output.status.success());
    let content = std::fs::read_to_string("deps_graph.dot").unwrap();
    assert!(content.contains("\"//b\" -> \"//a\""));

    std::fs::remove_file("deps_graph.dot").unwrap();
}
