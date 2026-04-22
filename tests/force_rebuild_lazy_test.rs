use redwood::datalog::{parser, Value};
use redwood::{datalog::Engine, runtime::prelude};
use std::time::Instant;

#[test]

fn force_rebuild_computed_lazily() {
    let num_targets = 2000;

    let mut build_content = String::new();

    for i in 0..num_targets {
        build_content.push_str(&format!("system_cc(\"//target:t{}\").\n", i));
        build_content.push_str(&format!("sources(\"//target:t{}\", \"t{}.c\").\n", i, i));
    }

    build_content.push_str("\nforce_rebuild(T) :- target(T).\n");

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(&build_content).unwrap();

    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let prelude_facts = prelude::get_prelude_facts();
    db.insert_facts(prelude_facts);
    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    eprintln!("Checking intermediate predicates:");
    let target_facts = db.query("target", &[Some("//target:t0")]);
    eprintln!("  target(//target:t0): {} (filtered)", target_facts.len());

    let target_all = db.query("target", &[]);
    eprintln!("  target(*): {} (unfiltered)", target_all.len());

    let force_rebuild_facts = db.query("force_rebuild", &[Some("//target:t0")]);
    eprintln!(
        "  force_rebuild(//target:t0): {} (filtered)",
        force_rebuild_facts.len()
    );

    let force_rebuild_all = db.query("force_rebuild", &[]);
    eprintln!(
        "  force_rebuild(*): {} (unfiltered)",
        force_rebuild_all.len()
    );

    let start = Instant::now();

    let needs_rebuild = db.query("needs_rebuild", &[Some("//target:t0")]);

    let elapsed = start.elapsed();
    eprintln!("  needs_rebuild(//target:t0): {}", needs_rebuild.len());

    assert_eq!(needs_rebuild.len(), 1);
    if let Some(Value::String(target)) = needs_rebuild[0].args.first() {
        assert_eq!(target, "//target:t0");
    } else {
        panic!("Expected needs_rebuild fact for //target:t0");
    }

    println!(
        "Query for 1 target (with filter) out of {} completed in {:?}",
        num_targets, elapsed
    );

    // Dependency resolution is now lazy - only queries transitive_deps for requested targets
    // This should complete quickly even with many targets in the workspace
    assert!(
        elapsed.as_secs() < 10,
        "Query should complete within reasonable time. Took {:?}",
        elapsed
    );
}

#[test]
fn force_rebuild_eager_would_be_slow() {
    let num_targets = 2000;

    let mut build_content = String::new();

    for i in 0..num_targets {
        build_content.push_str(&format!("system_cc(\"//target:t{}\").\n", i));
        build_content.push_str(&format!("sources(\"//target:t{}\", \"t{}.c\").\n", i, i));
    }

    build_content.push_str("\nforce_rebuild(T) :- target(T).\n");

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(&build_content).unwrap();

    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let prelude_facts = prelude::get_prelude_facts();
    db.insert_facts(prelude_facts);
    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let start = Instant::now();

    let needs_rebuild = db.query("needs_rebuild", &[]);

    let elapsed = start.elapsed();

    assert!(
        needs_rebuild.len() >= num_targets,
        "Should find needs_rebuild facts for all {} targets",
        num_targets
    );

    println!(
        "Query for all {} targets (no filter) completed in {:?}",
        num_targets, elapsed
    );

    // With 2000 targets, we expect O(N) behavior
    assert!(
        elapsed.as_secs() < 10,
        "Query should complete within reasonable time. Took {:?}",
        elapsed
    );
}

#[test]

fn force_rebuild_with_complex_graph_lazy() {
    let num_targets = 1000;

    let mut build_content = String::new();

    for i in 0..num_targets {
        build_content.push_str(&format!("system_cc(\"//lib:lib{}\").\n", i));
        build_content.push_str(&format!("sources(\"//lib:lib{}\", \"lib{}.c\").\n", i, i));

        if i > 0 {
            build_content.push_str(&format!(
                "deps(\"//lib:lib{}\", \"//lib:lib{}\").\n",
                i,
                i - 1
            ));
        }
    }

    build_content.push_str("\nsystem_cc(\"//app:isolated\").\n");
    build_content.push_str("sources(\"//app:isolated\", \"isolated.c\").\n");

    build_content.push_str("\nforce_rebuild(T) :- target(T).\n");

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(&build_content).unwrap();

    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let prelude_facts = prelude::get_prelude_facts();
    db.insert_facts(prelude_facts);
    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let start = Instant::now();

    let needs_rebuild = db.query("needs_rebuild", &[Some("//app:isolated")]);

    let elapsed = start.elapsed();

    assert_eq!(needs_rebuild.len(), 1);
    if let Some(Value::String(target)) = needs_rebuild[0].args.first() {
        assert_eq!(target, "//app:isolated");
    }

    println!(
        "Query for isolated target in graph of {} completed in {:?}",
        num_targets + 1,
        elapsed
    );

    // Note: Currently evaluates force_rebuild(T) :- target(T) for all T
    // This documents current behavior; filtering happens after rule evaluation
    assert!(
        elapsed.as_secs() < 10,
        "Query should complete within reasonable time. Took {:?}",
        elapsed
    );
}

#[test]

fn force_rebuild_specific_target_not_all() {
    let mut build_content = String::new();

    for i in 0..100 {
        build_content.push_str(&format!("system_cc(\"//lib:lib{}\").\n", i));
        build_content.push_str(&format!("sources(\"//lib:lib{}\", \"lib{}.c\").\n", i, i));
    }

    build_content.push_str("\nforce_rebuild(\"//lib:lib42\").\n");

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(&build_content).unwrap();

    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let prelude_facts = prelude::get_prelude_facts();
    db.insert_facts(prelude_facts);
    let prelude_rules = prelude::get_prelude_rules();
    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let needs_rebuild_42 = db.query("needs_rebuild", &[Some("//lib:lib42")]);
    assert_eq!(needs_rebuild_42.len(), 1);

    let needs_rebuild_0 = db.query("needs_rebuild", &[Some("//lib:lib0")]);
    assert_eq!(
        needs_rebuild_0.len(),
        0,
        "//lib:lib0 should not need rebuild - only //lib:lib42 was forced"
    );

    let needs_rebuild_99 = db.query("needs_rebuild", &[Some("//lib:lib99")]);
    assert_eq!(
        needs_rebuild_99.len(),
        0,
        "//lib:lib99 should not need rebuild - only //lib:lib42 was forced"
    );
}
