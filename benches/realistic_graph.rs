use redwood::datalog::Engine;
use redwood::datalog::{Fact, Predicate, Rule, Term, Value};
use std::time::Instant;

// Generate a realistic build graph similar to real repos:
// - Wide and shallow (not deep chains)
// - Each target depends on 5-15 libraries
// - Organized into layers: app -> services -> libs -> utils
fn generate_realistic_graph(num_targets: usize) -> (Vec<Fact>, Vec<Rule>) {
    let mut facts = Vec::new();
    let mut rules = Vec::new();

    // Layer sizes following 80/20 distribution
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
        facts.push(Fact {
            predicate: "kind".to_string(),
            args: vec![
                Value::String(target.clone()),
                Value::String("rust_library".to_string()),
            ],
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
        facts.push(Fact {
            predicate: "kind".to_string(),
            args: vec![
                Value::String(target.clone()),
                Value::String("rust_library".to_string()),
            ],
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
        facts.push(Fact {
            predicate: "kind".to_string(),
            args: vec![
                Value::String(target.clone()),
                Value::String("rust_library".to_string()),
            ],
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
        facts.push(Fact {
            predicate: "kind".to_string(),
            args: vec![
                Value::String(target.clone()),
                Value::String("rust_binary".to_string()),
            ],
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
    let scale = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1000);

    println!("Realistic Build Graph Benchmark");
    println!("================================");
    println!("Targets: {}", scale);

    let total_start = Instant::now();

    let (facts, rules) = generate_realistic_graph(scale);

    let num_deps = facts.iter().filter(|f| f.predicate == "deps").count();
    println!("Generated {} facts ({} direct deps)", facts.len(), num_deps);

    let mut db = Engine::new();

    let insert_start = Instant::now();
    db.insert_facts(facts);
    println!("Fact insertion: {:?}", insert_start.elapsed());

    let compile_start = Instant::now();
    for rule in rules {
        db.compile_rule(rule);
    }
    println!("Rule compilation: {:?}", compile_start.elapsed());

    println!("Total setup: {:?}", total_start.elapsed());

    let query_start = Instant::now();
    let results = db.query("transitive_deps", &[]);
    let query_time = query_start.elapsed();
    println!(
        "Transitive deps query: {:?} ({} results)",
        query_time,
        results.len()
    );

    let total_time = total_start.elapsed();
    println!("Total time: {:?}", total_time);
}
