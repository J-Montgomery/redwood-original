use redwood::datalog::Engine;
use redwood::datalog::{Fact, Predicate, Rule, Term, Value};

fn main() {
    let mut db = Engine::new();

    db.insert_facts(vec![
        Fact {
            predicate: "value".to_string(),
            args: vec![Value::String("a".to_string()), Value::Integer(1)],
        },
        Fact {
            predicate: "value".to_string(),
            args: vec![
                Value::String("b".to_string()),
                Value::String("text".to_string()),
            ],
        },
    ]);

    db.compile_rule(Rule {
        head: Predicate {
            name: "different".to_string(),
            args: vec![Term::Variable("X".to_string())],
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
                name: "!=".to_string(),
                args: vec![
                    Term::Variable("V".to_string()),
                    Term::Constant(Value::String("test".to_string())),
                ],
            },
        ],
    });

    db.query("different", &[]);
}
