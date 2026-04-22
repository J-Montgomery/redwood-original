use redwood::datalog::Engine;
use redwood::datalog::{Fact, Predicate, Rule, Term, Value};
use std::time::Instant;

fn generate_large_dataset(num_targets: usize) -> (Vec<Fact>, Vec<Rule>) {
    let mut facts = Vec::new();
    let mut rules = Vec::new();

    for i in 0..num_targets {
        let target = format!("//project{}:binary{}", i / 100, i % 100);

        facts.push(Fact {
            predicate: "target".to_string(),
            args: vec![Value::String(target.clone())],
        });

        facts.push(Fact {
            predicate: "kind".to_string(),
            args: vec![
                Value::String(target.clone()),
                Value::String("rust_binary".to_string()),
            ],
        });

        for j in 0..10 {
            facts.push(Fact {
                predicate: "src_file".to_string(),
                args: vec![
                    Value::String(target.clone()),
                    Value::String(format!("src/module{}/file{}.rs", i, j)),
                ],
            });
        }

        facts.push(Fact {
            predicate: "outputs".to_string(),
            args: vec![
                Value::String(target.clone()),
                Value::String(format!("target/release/binary{}", i)),
            ],
        });

        if i > 0 {
            let dep_target = format!("//project{}:binary{}", (i - 1) / 100, (i - 1) % 100);
            facts.push(Fact {
                predicate: "deps".to_string(),
                args: vec![Value::String(target.clone()), Value::String(dep_target)],
            });
        }
    }

    // sources(T, F) :- src_file(T, F)
    rules.push(Rule {
        head: Predicate {
            name: "sources".to_string(),
            args: vec![
                Term::Variable("T".to_string()),
                Term::Variable("F".to_string()),
            ],
        },
        body: vec![Predicate {
            name: "src_file".to_string(),
            args: vec![
                Term::Variable("T".to_string()),
                Term::Variable("F".to_string()),
            ],
        }],
    });

    // rust_target(T) :- kind(T, rust_binary)
    rules.push(Rule {
        head: Predicate {
            name: "rust_target".to_string(),
            args: vec![Term::Variable("T".to_string())],
        },
        body: vec![Predicate {
            name: "kind".to_string(),
            args: vec![
                Term::Variable("T".to_string()),
                Term::Constant(Value::String("rust_binary".to_string())),
            ],
        }],
    });

    // has_deps(T) :- deps(T, D)
    rules.push(Rule {
        head: Predicate {
            name: "has_deps".to_string(),
            args: vec![Term::Variable("T".to_string())],
        },
        body: vec![Predicate {
            name: "deps".to_string(),
            args: vec![
                Term::Variable("T".to_string()),
                Term::Variable("D".to_string()),
            ],
        }],
    });

    // transitive_deps(T, D) :- deps(T, D)
    rules.push(Rule {
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
    });

    // transitive_deps(T, D) :- deps(T, I), transitive_deps(I, D)
    rules.push(Rule {
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
    });

    (facts, rules)
}

fn benchmark_insertion(num_targets: usize) {
    println!("\n=== Benchmarking Insertion ({} targets) ===", num_targets);

    let (facts, rules) = generate_large_dataset(num_targets);
    println!("Generated {} facts, {} rules", facts.len(), rules.len());

    let mut db = Engine::new();

    let start = Instant::now();
    db.insert_facts(facts);
    let insert_time = start.elapsed();
    println!("Fact insertion: {:?}", insert_time);

    let start = Instant::now();
    for rule in rules {
        db.compile_rule(rule);
    }
    let compile_time = start.elapsed();
    println!("Rule compilation: {:?}", compile_time);
}

fn benchmark_queries(num_targets: usize) {
    println!("\n=== Benchmarking Queries ({} targets) ===", num_targets);

    let (facts, rules) = generate_large_dataset(num_targets);
    let mut db = Engine::new();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let start = Instant::now();
    let results = db.query("target", &[]);
    let query_time = start.elapsed();
    println!(
        "Query 'target': {:?} ({} results)",
        query_time,
        results.len()
    );

    let start = Instant::now();
    let results = db.query("sources", &[]);
    let query_time = start.elapsed();
    println!(
        "Query 'sources': {:?} ({} results)",
        query_time,
        results.len()
    );

    let start = Instant::now();
    let results = db.query("rust_target", &[]);
    let query_time = start.elapsed();
    println!(
        "Query 'rust_target': {:?} ({} results)",
        query_time,
        results.len()
    );

    let start = Instant::now();
    let results = db.query("transitive_deps", &[]);
    let query_time = start.elapsed();
    println!(
        "Query 'transitive_deps': {:?} ({} results)",
        query_time,
        results.len()
    );
}

fn benchmark_incremental_updates(num_targets: usize) {
    println!(
        "\n=== Benchmarking Incremental Updates ({} targets) ===",
        num_targets
    );

    let (facts, rules) = generate_large_dataset(num_targets);
    let mut db = Engine::new();
    db.insert_facts(facts.clone());
    for rule in rules {
        db.compile_rule(rule);
    }

    let facts_to_update: Vec<Fact> = facts.iter().take(100).cloned().collect();

    let start = Instant::now();
    db.retract_facts(facts_to_update.clone());
    let retract_time = start.elapsed();
    println!("Retract 100 facts: {:?}", retract_time);

    let start = Instant::now();
    db.insert_facts(facts_to_update);
    let reinsert_time = start.elapsed();
    println!("Re-insert 100 facts: {:?}", reinsert_time);
}

fn benchmark_scalability() {
    println!("\n=== Scalability Test ===");

    for scale in [100, 1000, 10000] {
        println!("\n--- Scale: {} targets ---", scale);

        let total_start = Instant::now();

        let (facts, rules) = generate_large_dataset(scale);
        let mut db = Engine::new();
        db.insert_facts(facts);
        for rule in rules {
            db.compile_rule(rule);
        }

        let total_time = total_start.elapsed();
        println!("Total setup time: {:?}", total_time);

        let query_start = Instant::now();
        let results = db.query("transitive_deps", &[]);
        let query_time = query_start.elapsed();
        println!("Query time: {:?} ({} results)", query_time, results.len());
    }
}

fn benchmark_large_scale(scale: usize) {
    println!("\n=== Large Scale Test ({} targets) ===", scale);

    let total_start = Instant::now();

    let (facts, rules) = generate_large_dataset(scale);
    println!("Generated {} facts, {} rules", facts.len(), rules.len());

    let mut db = Engine::new();

    let insert_start = Instant::now();
    db.insert_facts(facts);
    let insert_time = insert_start.elapsed();
    println!("Fact insertion: {:?}", insert_time);

    let compile_start = Instant::now();
    for rule in rules {
        db.compile_rule(rule);
    }
    let compile_time = compile_start.elapsed();
    println!("Rule compilation: {:?}", compile_time);

    let total_time = total_start.elapsed();
    println!("Total setup time: {:?}", total_time);

    let query_start = Instant::now();
    let results = db.query("transitive_deps", &[]);
    let query_time = query_start.elapsed();
    println!(
        "Transitive deps query: {:?} ({} results)",
        query_time,
        results.len()
    );
}

fn main() {
    println!("Redwood Performance Benchmark");
    println!("==============================");

    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("standard");

    match mode {
        "quick" => {
            println!("Running quick benchmarks");
            benchmark_insertion(100);
            benchmark_queries(100);
            benchmark_incremental_updates(100);
        }
        "large" => {
            let scale = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(100000);
            benchmark_large_scale(scale);
        }
        _ => {
            let (facts, _) = generate_large_dataset(10);
            let mut db = Engine::new();
            db.insert_facts(facts);


            benchmark_insertion(1000);
            benchmark_queries(1000);
            benchmark_incremental_updates(1000);
            benchmark_scalability();
        }
    }

    println!("\n=== Benchmark Complete ===");
}
