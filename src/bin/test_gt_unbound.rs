use redwood::datalog::{parser, Engine};

fn main() {
    let program = r#"
        value(10).
        bad_result(X) :- value(X), gt(X, Y).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let _results = db.query("bad_result", &[]);
}
