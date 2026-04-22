use redwood::datalog::Engine;
use redwood::datalog::{Fact, Predicate, Rule, Term, Value};
use std::time::Instant;

fn setup_database(num_targets: usize) -> (Engine, Vec<String>) {
    let mut db = Engine::new();
    let mut targets = Vec::new();

    let mut facts = Vec::new();
    for i in 0..num_targets {
        let target = format!("//pkg:lib{}", i);
        facts.push(Fact {
            predicate: "target".to_string(),
            args: vec![Value::String(target.clone())],
        });
        facts.push(Fact {
            predicate: "kind".to_string(),
            args: vec![
                Value::String(target.clone()),
                Value::String(
                    if i % 3 == 0 {
                        "rust_binary"
                    } else {
                        "rust_library"
                    }
                    .to_string(),
                ),
            ],
        });

        if i > 0 {
            let num_deps = 1 + (i % 5);
            for j in 0..num_deps {
                let dep_idx = (i - j - 1).max(0);
                facts.push(Fact {
                    predicate: "deps".to_string(),
                    args: vec![
                        Value::String(target.clone()),
                        Value::String(format!("//pkg:lib{}", dep_idx)),
                    ],
                });
            }
        }

        targets.push(target);
    }

    db.insert_facts(facts);

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

    for rule in rules {
        db.compile_rule(rule);
    }

    (db, targets)
}

fn benchmark_pattern(name: &str, num_targets: usize, query_fn: impl Fn(&mut Engine, &[String])) {
    println!("\n--- {} ({} targets) ---", name, num_targets);

    let (mut db, targets) = setup_database(num_targets);

    let start = Instant::now();
    query_fn(&mut db, &targets);
    let elapsed = start.elapsed();

    println!("Time: {:?}", elapsed);

    if elapsed.as_millis() < 1 {
        println!("✓ Sub-millisecond");
    } else if elapsed.as_millis() < 100 {
        println!("✓ Fast (<100ms)");
    } else {
        println!("✗ Slow (>100ms)");
    }
}

fn main() {
    println!("Query Pattern Benchmark");
    println!("=======================");
    println!("Spec Goal: <1ms simple, <100ms complex\n");

    let scales = vec![1_000, 10_000, 100_000];

    for scale in scales {
        println!("\n=== {} targets ===", scale);

        benchmark_pattern("Base fact lookup", scale, |db, _targets| {
            let _ = db.query("target", &[]);
        });

        benchmark_pattern("Filtered base fact", scale, |db, targets| {
            let _ = db.query("kind", &[Some(&targets[0]), None]);
        });

        benchmark_pattern("Join query (target + kind)", scale, |db, targets| {
            for target in targets {
                let _ = db.query("kind", &[Some(target), None]);
            }
        });

        benchmark_pattern("Single target TC (lazy)", scale, |db, targets| {
            let mid = targets.len() / 2;
            let _ = db.query("transitive_deps", &[Some(&targets[mid]), None]);
        });

        benchmark_pattern(
            "Fanout query (all deps of one target)",
            scale,
            |db, targets| {
                let last = targets.len() - 1;
                let _ = db.query("deps", &[Some(&targets[last]), None]);
            },
        );

        benchmark_pattern("Fanin query (all dependents)", scale, |db, targets| {
            let _ = db.query("deps", &[None, Some(&targets[0])]);
        });
    }

    println!("\n=== Analysis ===");
    println!("Query patterns by complexity:");
    println!("1. Base fact lookup: O(1) with indexing");
    println!("2. Filtered fact: O(n) scan or O(1) with index");
    println!("3. Join: O(n*m) - bottleneck for large graphs");
    println!("4. Lazy TC: O(deps) - only compute needed portion");
    println!("\nOptimization priorities:");
    println!("1. Index on first argument of predicates (biggest win)");
    println!("2. Cache lazy TC results per target");
    println!("3. Pre-compute common queries");
    println!("\nNote: Full TC not included (O(n³), doesn't scale)");
    println!("See full_tc_small.rs for full TC on ≤100 targets");
}
