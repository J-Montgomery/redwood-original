use redwood::datalog::Engine;
use redwood::datalog::{Fact, Predicate, Rule, Term, Value};

fn main() {
    let mut db = Engine::new();

    db.insert_facts(vec![Fact {
        predicate: "value".to_string(),
        args: vec![Value::String("a".to_string()), Value::Integer(42)],
    }]);

    db.compile_rule(Rule {
        head: Predicate {
            name: "result".to_string(),
            args: vec![
                Term::Variable("X".to_string()),
                Term::Variable("R".to_string()),
            ],
        },
        body: vec![
            Predicate {
                name: "value".to_string(),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("V".to_string()),
                ],
            },
            Predicate {
                name: "concat".to_string(),
                args: vec![
                    Term::Variable("V".to_string()),
                    Term::Constant(Value::Integer(10)),
                    Term::Variable("R".to_string()),
                ],
            },
        ],
    });

    db.query("result", &[]);
}
