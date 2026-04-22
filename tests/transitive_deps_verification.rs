use redwood::datalog::Engine;
use redwood::datalog::{Fact, Predicate, Rule, Term, Value};

#[test]
fn verify_transitive_deps_count() {
    let mut facts = Vec::new();

    // Create chain: 0 -> 1 -> 2 -> 3 -> 4
    for i in 0..5 {
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
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("transitive_deps", &[]);
    println!("Results: {:?}", results);

    // Verify we get the expected 10 transitive deps for a 5-node chain
    assert_eq!(
        results.len(),
        10,
        "Expected 10 transitive deps for 5-node chain"
    );
}

#[test]
fn verify_larger_transitive_chain() {
    let mut facts = Vec::new();

    // Create longer chain with 10 targets
    for i in 0..10 {
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
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query("transitive_deps", &[]);
    println!("Results count: {}", results.len());

    // 10 targets in chain: sum(1..9) = 45 transitive deps
    assert_eq!(
        results.len(),
        45,
        "Expected 45 transitive deps for 10-node chain"
    );
}
