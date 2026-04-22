use redwood::datalog::Engine;
use redwood::datalog::{Fact, Predicate, Rule, Term, Value};
use std::time::Instant;

fn setup_graph(num_targets: usize) -> (Engine, Vec<String>) {
    let mut db = Engine::new();
    let mut targets = Vec::new();

    let mut facts = Vec::new();
    for i in 0..num_targets {
        let target = format!("//pkg:target{}", i);
        facts.push(Fact {
            predicate: "target".to_string(),
            args: vec![Value::String(target.clone())],
        });

        if i > 0 {
            facts.push(Fact {
                predicate: "deps".to_string(),
                args: vec![
                    Value::String(target.clone()),
                    Value::String(format!("//pkg:target{}", i - 1)),
                ],
            });
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

fn benchmark_incremental_insert(num_targets: usize) {
    println!("\n=== Incremental Insert ({} targets) ===", num_targets);

    let (mut db, targets) = setup_graph(num_targets);

    let baseline_start = Instant::now();
    let baseline_results = db.query(
        "transitive_deps",
        &[Some(&targets[targets.len() - 1]), None],
    );
    let baseline_time = baseline_start.elapsed();
    println!(
        "Baseline query: {:?} ({} deps)",
        baseline_time,
        baseline_results.len()
    );

    let new_target = format!("//pkg:target{}", num_targets);
    let new_facts = vec![
        Fact {
            predicate: "target".to_string(),
            args: vec![Value::String(new_target.clone())],
        },
        Fact {
            predicate: "deps".to_string(),
            args: vec![
                Value::String(new_target.clone()),
                Value::String(targets[targets.len() - 1].clone()),
            ],
        },
    ];

    let insert_start = Instant::now();
    db.insert_facts(new_facts);
    let insert_time = insert_start.elapsed();
    println!("Insert new target: {:?}", insert_time);

    // Query the OLD target again (should be cached)
    let requery_start = Instant::now();
    let requery_results = db.query(
        "transitive_deps",
        &[Some(&targets[targets.len() - 1]), None],
    );
    let requery_time = requery_start.elapsed();
    println!(
        "Re-query old target: {:?} ({} deps)",
        requery_time,
        requery_results.len()
    );

    // Query the NEW target (will need computation)
    let query_start = Instant::now();
    let new_results = db.query("transitive_deps", &[Some(&new_target), None]);
    let query_time = query_start.elapsed();
    println!(
        "Query new target: {:?} ({} deps)",
        query_time,
        new_results.len()
    );

    // Check incrementality: re-query should be much faster than baseline
    if requery_time < baseline_time / 3 {
        println!(
            "✓ Incremental: old target cached ({}x faster)",
            baseline_time.as_nanos() / requery_time.as_nanos()
        );
    } else {
        println!("✗ Not incremental: old target recomputed");
    }
}

fn benchmark_fact_retraction(num_targets: usize) {
    println!("\n=== Fact Retraction ({} targets) ===", num_targets);

    let (mut db, targets) = setup_graph(num_targets);

    let before_start = Instant::now();
    let before_results = db.query(
        "transitive_deps",
        &[Some(&targets[targets.len() - 1]), None],
    );
    println!(
        "Before: {:?} ({} deps)",
        before_start.elapsed(),
        before_results.len()
    );

    let middle_idx = num_targets / 2;
    let facts_to_retract = vec![Fact {
        predicate: "deps".to_string(),
        args: vec![
            Value::String(targets[middle_idx].clone()),
            Value::String(targets[middle_idx - 1].clone()),
        ],
    }];

    let retract_start = Instant::now();
    db.retract_facts(facts_to_retract);
    let retract_time = retract_start.elapsed();
    println!("Retract fact: {:?}", retract_time);

    let after_start = Instant::now();
    let after_results = db.query(
        "transitive_deps",
        &[Some(&targets[targets.len() - 1]), None],
    );
    let after_time = after_start.elapsed();
    println!("After: {:?} ({} deps)", after_time, after_results.len());

    println!("Total: {:?}", retract_time + after_time);
}

fn benchmark_rule_compilation(num_rules: usize) {
    println!("\n=== Rule Compilation ({} rules) ===", num_rules);

    let mut db = Engine::new();

    let facts = vec![Fact {
        predicate: "base".to_string(),
        args: vec![
            Value::String("a".to_string()),
            Value::String("b".to_string()),
        ],
    }];
    db.insert_facts(facts);

    let compile_start = Instant::now();
    for i in 0..num_rules {
        let rule = Rule {
            head: Predicate {
                name: format!("derived{}", i),
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("Y".to_string()),
                ],
            },
            body: vec![Predicate {
                name: if i == 0 {
                    "base".to_string()
                } else {
                    format!("derived{}", i - 1)
                },
                args: vec![
                    Term::Variable("X".to_string()),
                    Term::Variable("Y".to_string()),
                ],
            }],
        };
        db.compile_rule(rule);
    }
    let compile_time = compile_start.elapsed();

    println!("Compile {} rules: {:?}", num_rules, compile_time);
    let per_rule = compile_time.as_micros() / num_rules as u128;
    println!("Per rule: {} μs", per_rule);

    if per_rule < 1000 {
        println!("✓ Sub-millisecond rule compilation");
    } else {
        println!("✗ Slow rule compilation");
    }
}

fn main() {
    println!("Incremental Operations Benchmark");
    println!("=================================");
    println!("Goal: Incremental updates without full recomputation\n");

    let scales = vec![100, 1_000, 10_000];

    for scale in &scales {
        benchmark_incremental_insert(*scale);
    }

    for scale in &scales {
        benchmark_fact_retraction(*scale);
    }

    let rule_counts = vec![10, 100, 1_000];
    for count in rule_counts {
        benchmark_rule_compilation(count);
    }

    println!("\n=== Analysis ===");
    println!("True incremental systems only recompute affected portions.");
    println!("With selective TC cache invalidation, unaffected queries stay cached.");
}
