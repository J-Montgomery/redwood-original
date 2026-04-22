use redwood::datalog::{parser, Engine, Value};

#[test]
fn mutual_recursion_terminates() {
    let program = r#"
        foo("a").
        foo(X) :- bar(X).
        bar(X) :- foo(X).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("foo", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0], Value::String("a".to_string()));

    let bar_results = db.query("bar", &[]);
    assert_eq!(bar_results.len(), 1);
    assert_eq!(bar_results[0].args[0], Value::String("a".to_string()));
}

#[test]
fn self_referential_rule_terminates() {
    let program = r#"
        node("a").
        node("b").
        node(X) :- node(X).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("node", &[]);
    assert_eq!(results.len(), 2);
}

#[test]
fn three_way_cycle_terminates() {
    let program = r#"
        a("start").
        a(X) :- b(X).
        b(X) :- c(X).
        c(X) :- a(X).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results_a = db.query("a", &[]);
    let results_b = db.query("b", &[]);
    let results_c = db.query("c", &[]);

    assert_eq!(results_a.len(), 1);
    assert_eq!(results_b.len(), 1);
    assert_eq!(results_c.len(), 1);
}

#[test]
fn transitive_closure_with_cycle() {
    let program = r#"
        edge("a", "b").
        edge("b", "c").
        edge("c", "a").
        path(X, Y) :- edge(X, Y).
        path(X, Z) :- edge(X, Y), path(Y, Z).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("path", &[]);
    assert!(results.len() >= 3);

    let has_cycle = results.iter().any(|f| {
        if f.args.len() == 2 {
            f.args[0] == Value::String("a".to_string())
                && f.args[1] == Value::String("a".to_string())
        } else {
            false
        }
    });
    assert!(has_cycle);
}

#[test]
fn mutual_recursion_with_filtering() {
    let program = r#"
        foo(1).
        foo(2).
        foo(X) :- bar(X), lt(X, 10).
        bar(X) :- foo(X), lt(X, 5).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let foo_results = db.query("foo", &[]);
    assert_eq!(foo_results.len(), 2);

    let bar_results = db.query("bar", &[]);
    assert_eq!(bar_results.len(), 2);
}

#[test]
fn empty_cycle_produces_no_results() {
    let program = r#"
        foo(X) :- bar(X).
        bar(X) :- baz(X).
        baz(X) :- foo(X).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let foo_results = db.query("foo", &[]);
    let bar_results = db.query("bar", &[]);
    let baz_results = db.query("baz", &[]);

    assert_eq!(foo_results.len(), 0);
    assert_eq!(bar_results.len(), 0);
    assert_eq!(baz_results.len(), 0);
}
