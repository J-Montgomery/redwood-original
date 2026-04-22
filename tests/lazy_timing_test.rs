use redwood::datalog::Engine;
use redwood::datalog::{Fact, Predicate, Rule, Term, Value};
use std::time::Instant;

#[test]
fn verify_lazy_evaluation_performance() {
    let mut facts = Vec::new();

    for i in 0..100 {
        let target = format!("//target{}", i);
        facts.push(Fact {
            predicate: "target".to_string(),
            args: vec![Value::String(target.clone())],
        });

        if i > 0 {
            let dep = format!("//target{}", i - 1);
            facts.push(Fact {
                predicate: "deps".to_string(),
                args: vec![Value::String(target.clone()), Value::String(dep)],
            });
        }
    }

    let rules = vec![
        // transitive_deps(T, D) :- deps(T, D)
        Rule {
            head: Predicate {
                name: "transitive_deps".to_string(),
                args: vec![
                    Term::Variable("T".to_string()),
                    Term::Variable("D".to_string()),
                ],
            },
            body: vec![Predicate {
                name: "deps".to_string(),
                args: vec![
                    Term::Variable("T".to_string()),
                    Term::Variable("D".to_string()),
                ],
            }],
        },
        // transitive_deps(T, D) :- deps(T, I), transitive_deps(I, D)
        Rule {
            head: Predicate {
                name: "transitive_deps".to_string(),
                args: vec![
                    Term::Variable("T".to_string()),
                    Term::Variable("D".to_string()),
                ],
            },
            body: vec![
                Predicate {
                    name: "deps".to_string(),
                    args: vec![
                        Term::Variable("T".to_string()),
                        Term::Variable("I".to_string()),
                    ],
                },
                Predicate {
                    name: "transitive_deps".to_string(),
                    args: vec![
                        Term::Variable("I".to_string()),
                        Term::Variable("D".to_string()),
                    ],
                },
            ],
        },
    ];

    let mut db = Engine::new();

    let insert_start = Instant::now();
    db.insert_facts(facts);
    let insert_time = insert_start.elapsed();
    println!("Insert facts: {:?}", insert_time);

    let compile_start = Instant::now();
    for rule in rules {
        db.compile_rule(rule);
    }
    let compile_time = compile_start.elapsed();

    let query_start = Instant::now();
    let results = db.query("transitive_deps", &[]);
    let query_time = query_start.elapsed();
    println!(
        "Query transitive_deps (lazy evaluation): {:?} ({} results)",
        query_time,
        results.len()
    );

    println!("Total: {:?}", compile_time + query_time);

    assert_eq!(results.len(), 4950);

    // Rule compilation should be <1ms
    assert!(
        compile_time.as_millis() < 1,
        "Rule compilation took too long: {:?}. Should be <1ms for lazy evaluation",
        compile_time
    );
}
