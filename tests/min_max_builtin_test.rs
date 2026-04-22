use redwood::datalog::{parser, Engine, Value};

#[test]
fn min_finds_minimum_integer() {
    let mut db = Engine::new();

    let program = r#"
priority("//app:a", 5).
priority("//app:b", 2).
priority("//app:c", 8).

lowest_priority(P) :- min("priority", 1, P).
"#;

    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("lowest_priority", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0], Value::Integer(2));
}

#[test]
fn max_finds_maximum_integer() {
    let mut db = Engine::new();

    let program = r#"
priority("//app:a", 5).
priority("//app:b", 2).
priority("//app:c", 8).

highest_priority(P) :- max("priority", 1, P).
"#;

    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("highest_priority", &[]);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].args[0], Value::Integer(8));
}

#[test]
fn min_with_bound_value() {
    let mut db = Engine::new();

    let program = r#"
score("//app:a", 10).
score("//app:b", 5).
score("//app:c", 15).

has_min_five() :- min("score", 1, 5).
has_min_three() :- min("score", 1, 3).
"#;

    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let five = db.query("has_min_five", &[]);
    assert_eq!(five.len(), 1);

    let three = db.query("has_min_three", &[]);
    assert_eq!(three.len(), 0);
}

#[test]
fn max_with_bound_value() {
    let mut db = Engine::new();

    let program = r#"
score("//app:a", 10).
score("//app:b", 5).
score("//app:c", 15).

has_max_fifteen() :- max("score", 1, 15).
has_max_twenty() :- max("score", 1, 20).
"#;

    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let fifteen = db.query("has_max_fifteen", &[]);
    assert_eq!(fifteen.len(), 1);

    let twenty = db.query("has_max_twenty", &[]);
    assert_eq!(twenty.len(), 0);
}

#[test]
fn min_max_empty_predicate() {
    let mut db = Engine::new();

    let program = r#"
target("//app:a").

min_priority(P) :- min("nonexistent", 0, P).
max_priority(P) :- max("nonexistent", 0, P).
"#;

    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let min_results = db.query("min_priority", &[]);
    assert_eq!(min_results.len(), 0);

    let max_results = db.query("max_priority", &[]);
    assert_eq!(max_results.len(), 0);
}

#[test]
fn min_max_with_single_value() {
    let mut db = Engine::new();

    let program = r#"
value("only", 42).

min_val(V) :- min("value", 1, V).
max_val(V) :- max("value", 1, V).
"#;

    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let min_results = db.query("min_val", &[]);
    assert_eq!(min_results.len(), 1);
    assert_eq!(min_results[0].args[0], Value::Integer(42));

    let max_results = db.query("max_val", &[]);
    assert_eq!(max_results.len(), 1);
    assert_eq!(max_results[0].args[0], Value::Integer(42));
}

#[test]
fn min_max_different_arg_positions() {
    let mut db = Engine::new();

    let program = r#"
data("a", 10, 100).
data("b", 20, 50).
data("c", 5, 200).

min_first(V) :- min("data", 1, V).
max_first(V) :- max("data", 1, V).
min_second(V) :- min("data", 2, V).
max_second(V) :- max("data", 2, V).
"#;

    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let min_first = db.query("min_first", &[]);
    assert_eq!(min_first[0].args[0], Value::Integer(5));

    let max_first = db.query("max_first", &[]);
    assert_eq!(max_first[0].args[0], Value::Integer(20));

    let min_second = db.query("min_second", &[]);
    assert_eq!(min_second[0].args[0], Value::Integer(50));

    let max_second = db.query("max_second", &[]);
    assert_eq!(max_second[0].args[0], Value::Integer(200));
}
