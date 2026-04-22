use redwood::datalog::Engine;
use redwood::datalog::{Fact, Predicate, Rule, Term, Value};
use std::time::Instant;

fn generate_realistic_graph(num_targets: usize) -> (Vec<Fact>, Vec<Rule>) {
    let mut facts = Vec::new();
    let mut rules = Vec::new();

    let num_apps = num_targets / 20;
    let num_services = num_targets / 10;
    let num_libs = num_targets / 2;
    let num_utils = num_targets - num_apps - num_services - num_libs;

    let mut target_id = 0;
    let mut all_targets = Vec::new();

    let utils_start = target_id;
    for _ in 0..num_utils {
        let target = format!("//utils:util{}", target_id);
        facts.push(Fact {
            predicate: "target".to_string(),
            args: vec![Value::String(target.clone())],
        });
        all_targets.push(target);
        target_id += 1;
    }
    let utils_end = target_id;

    let libs_start = target_id;
    for _ in 0..num_libs {
        let target = format!("//lib:lib{}", target_id);
        facts.push(Fact {
            predicate: "target".to_string(),
            args: vec![Value::String(target.clone())],
        });

        let num_deps = 3 + (target_id * 7) % 6;
        for j in 0..num_deps {
            let dep_idx = utils_start + ((target_id * 13 + j * 17) % (utils_end - utils_start));
            facts.push(Fact {
                predicate: "deps".to_string(),
                args: vec![
                    Value::String(target.clone()),
                    Value::String(all_targets[dep_idx].clone()),
                ],
            });
        }

        all_targets.push(target);
        target_id += 1;
    }
    let libs_end = target_id;

    let services_start = target_id;
    for _ in 0..num_services {
        let target = format!("//service:svc{}", target_id);
        facts.push(Fact {
            predicate: "target".to_string(),
            args: vec![Value::String(target.clone())],
        });

        let num_deps = 5 + (target_id * 11) % 8;
        for j in 0..num_deps {
            let dep_idx = libs_start + ((target_id * 19 + j * 23) % (libs_end - libs_start));
            facts.push(Fact {
                predicate: "deps".to_string(),
                args: vec![
                    Value::String(target.clone()),
                    Value::String(all_targets[dep_idx].clone()),
                ],
            });
        }

        all_targets.push(target);
        target_id += 1;
    }
    let services_end = target_id;

    for _ in 0..num_apps {
        let target = format!("//app:app{}", target_id);
        facts.push(Fact {
            predicate: "target".to_string(),
            args: vec![Value::String(target.clone())],
        });

        let num_deps = 8 + (target_id * 7) % 8;
        for j in 0..num_deps {
            let dep_idx = if j % 3 == 0 {
                services_start + ((target_id * 29 + j * 31) % (services_end - services_start))
            } else {
                libs_start + ((target_id * 37 + j * 41) % (libs_end - libs_start))
            };
            facts.push(Fact {
                predicate: "deps".to_string(),
                args: vec![
                    Value::String(target.clone()),
                    Value::String(all_targets[dep_idx].clone()),
                ],
            });
        }

        all_targets.push(target);
        target_id += 1;
    }

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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let scale = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(100000);

    println!("Lazy TC Benchmark - Realistic Build Graph");
    println!("==========================================");
    println!("Targets: {}", scale);

    let (facts, rules) = generate_realistic_graph(scale);
    let num_deps = facts.iter().filter(|f| f.predicate == "deps").count();
    println!("Generated {} facts ({} direct deps)", facts.len(), num_deps);

    let mut db = Engine::new();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }
    println!("Setup complete");
    println!("\n--- Test 1: Lazy single-target query ---");
    let target_name = format!("//app:app{}", scale - 1);

    let start = Instant::now();
    let results = db.query("transitive_deps", &[Some(&target_name), None]);
    let query_time = start.elapsed();
    println!(
        "Query transitive_deps({}, _): {:?} ({} results)",
        target_name,
        query_time,
        results.len()
    );

    println!("\n--- Test 2: Multiple lazy queries (with cache) ---");
    let start = Instant::now();
    let mut total_results = 0;
    for i in 0..10 {
        let target = format!("//app:app{}", scale - 1 - i);
        let results = db.query("transitive_deps", &[Some(&target), None]);
        total_results += results.len();
    }
    let query_time = start.elapsed();
    println!(
        "10 queries: {:?} ({} total results, {} avg)",
        query_time,
        total_results,
        total_results / 10
    );

    println!("\n--- Test 3: Lazy iterator (true lazy evaluation) ---");
    let target_name = format!("//app:app{}", scale - 1);

    let start = Instant::now();
    let mut iter = db.query_tc_iter("transitive_deps", &target_name);
    let first_10: Vec<_> = iter.by_ref().take(10).collect();
    let first_10_time = start.elapsed();
    println!(
        "Iterator first 10 results: {:?} ({} results)",
        first_10_time,
        first_10.len()
    );

    let start = Instant::now();
    let remaining = iter.count();
    let count_time = start.elapsed();
    println!(
        "Count remaining: {:?} ({} more results)",
        count_time,
        remaining
    );

    let start = Instant::now();
    let iter = db.query_tc_iter("transitive_deps", &target_name);
    let all_results: Vec<_> = iter.collect();
    let collect_time = start.elapsed();
    println!(
        "Iterator collect all: {:?} ({} total results)",
        collect_time,
        all_results.len()
    );

    // Test 4: Full TC (eager - computes everything)
    // println!("\n--- Test 4: Full transitive closure (eager) ---");
    // let start = Instant::now();
    // let results = db.query("transitive_deps", &[]);
    // let query_time = start.elapsed();
    // println!(
    //     "Query transitive_deps(_, _): {:?} ({} results)",
    //     query_time,
    //     results.len()
    // );

    println!("\n=== Comparison ===");
    println!("Lazy iterator allows computing only what's needed,");
    println!("making targeted queries much faster than full TC.");
    println!("The iterator can be consumed incrementally without");
    println!("materializing all results upfront.");
}
