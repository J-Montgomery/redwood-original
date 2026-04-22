use redwood::datalog::{parser, Engine};

/// Test what actually happens when a variable appears only in negation
/// and not in positive body literals.
///
/// Example: result(X) :- foo(X), not(bar(Z)).
#[test]
fn test_ungrounded_in_negation_behavior() {
    let program = r#"
        foo("a").
        foo("b").
        bar("x").
        bar("y").

        % Z is ungrounded - only appears in negation
        result(X) :- foo(X), not(bar(Z)).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("result", &[]);

    // What do we get?
    println!("\n=== Test: Ungrounded variable in negation ===");
    println!("Facts: foo(a), foo(b), bar(x), bar(y)");
    println!("Rule: result(X) :- foo(X), not(bar(Z))");
    println!("Results: {} tuples", results.len());
    for fact in &results {
        println!("  {:?}", fact);
    }

    assert_eq!(results.len(), 0);
}

#[test]
fn test_negation_with_no_matching_facts() {
    let program = r#"
        foo("a").
        foo("b").

        % Z is ungrounded - only appears in negation
        % But there are NO bar facts
        result(X) :- foo(X), not(bar(Z)).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("result", &[]);

    println!("\n=== Test: Ungrounded in negation, no matching facts ===");
    println!("Facts: foo(a), foo(b), NO bar facts");
    println!("Rule: result(X) :- foo(X), not(bar(Z))");
    println!("Results: {} tuples", results.len());
    for fact in &results {
        println!("  {:?}", fact);
    }

    assert_eq!(results.len(), 2);
}

// NOTE: This test is commented out because compile_rule calls std::process::exit(1)
// instead of panicking, which makes it impossible to test with #[should_panic].
// The safety checker DOES correctly reject this rule because Z appears in the head
// but not in any positive body literal.
//
// #[test]
// fn test_negation_cartesian_product() {
//     let program = r#"
//         foo("a").
//         foo("b").
//         bar("x").
//         bar("y").
//         cartesian(X, Z) :- foo(X), not(bar(Z)).
//     "#;
//
//     let mut db = Engine::new();
//     let (facts, rules) = parser::parse_program(program).unwrap();
//     db.insert_facts(facts);
//     for rule in rules {
//         db.compile_rule(rule);
//     }
// }

#[test]
fn test_correct_negation_usage() {
    let program = r#"
        foo("a").
        foo("b").
        bar("a").

        % Correct: Z is bound by foo, used in negation
        result(X) :- foo(X), not(bar(X)).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("result", &[]);

    println!("\n=== Test: Correct negation usage ===");
    println!("Facts: foo(a), foo(b), bar(a)");
    println!("Rule: result(X) :- foo(X), not(bar(X))");
    println!("Results: {} tuples", results.len());
    for fact in &results {
        println!("  {:?}", fact);
    }

    assert_eq!(results.len(), 1);
    if let redwood::datalog::Value::String(s) = &results[0].args[0] {
        assert_eq!(s, "b");
    } else {
        panic!("Expected string value");
    }
}

#[test]
fn test_complex_ungrounded_negation() {
    let program = r#"
        node("a").
        node("b").
        edge("a", "b").

        % Has isolated nodes: nodes with no outgoing edges
        % This is WRONG: Y is ungrounded in negation
        isolated(X) :- node(X), not(edge(X, Y)).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("isolated", &[]);

    println!("\n=== Test: Complex ungrounded negation (graph example) ===");
    println!("Facts: node(a), node(b), edge(a,b)");
    println!("Rule: isolated(X) :- node(X), not(edge(X, Y))");
    println!("Results: {} tuples", results.len());
    for fact in &results {
        println!("  {:?}", fact);
    }

    assert_eq!(results.len(), 1);
    if let redwood::datalog::Value::String(s) = &results[0].args[0] {
        assert_eq!(s, "b");
    } else {
        panic!("Expected string value");
    }
}

#[test]
fn test_correct_isolated_nodes() {
    let program = r#"
        node("a").
        node("b").
        edge("a", "b").

        % Correct way: use anonymous variable
        isolated(X) :- node(X), not(edge(X, _)).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("isolated", &[]);

    assert_eq!(results.len(), 1);
    if let redwood::datalog::Value::String(s) = &results[0].args[0] {
        assert_eq!(s, "b");
    } else {
        panic!("Expected string value");
    }
}
