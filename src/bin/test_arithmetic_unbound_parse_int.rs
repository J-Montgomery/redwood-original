use redwood::datalog::{parser, Engine};

fn main() {
    let program = r#"
        expected(42).
        parsed(Str) :- expected(N), parse_int(Str, N).
    "#;

    let mut db = Engine::new();
    let (facts, rules) = parser::parse_program(program).unwrap();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let _results = db.query("parsed", &[]);
}
