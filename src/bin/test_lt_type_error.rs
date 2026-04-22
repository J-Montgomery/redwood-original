use redwood::datalog::Engine;
use redwood::datalog::{Fact, Predicate, Rule, Term, Value};

fn main() {
    let mut db = Engine::new();

    db.insert_facts(vec![Fact {
        predicate: "value".to_string(),
        args: vec![
            Value::String("a".to_string()),
            Value::String("apple".to_string()),
        ],
    }]);

    db.compile_rule(Rule {
        head: Predicate {
            name: "low_value".to_string(),
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
                name: "lt".to_string(),
                args: vec![
                    Term::Variable("V".to_string()),
                    Term::Constant(Value::String("apple".to_string())),
                ],
            },
        ],
    });

    db.query("low_value", &[]);
}
