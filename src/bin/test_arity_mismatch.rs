use redwood::datalog::{parser, Engine};

fn main() {
    let program = r#"
        foo(X) :- bar(X).
        foo(X, Y) :- baz(X, Y).
    "#;

    let mut db = Engine::new();
    let (_, rules) = parser::parse_program(program).unwrap();
    for rule in rules {
        db.compile_rule(rule);
    }
}
