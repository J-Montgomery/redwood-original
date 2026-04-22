use redwood::datalog::{parser, Engine};

fn main() {
    let program = r#"
        base("a").
        p(X) :- q(X).
        q(X) :- base(X), not(p(X)).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }
}
