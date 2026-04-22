use redwood::datalog::Engine;
use redwood::datalog::{Fact, Predicate, Rule, Term, Value};
use std::time::Instant;

fn generate_realistic_graph(num_targets: usize) -> (Vec<Fact>, Vec<Rule>, Vec<String>) {
    let mut facts = Vec::new();
    let mut rules = Vec::new();

    let num_apps = num_targets / 20;
    let num_services = num_targets / 10;
    let num_libs = num_targets / 2;
    let num_utils = num_targets - num_apps - num_services - num_libs;

    let mut target_id = 0;
    let mut all_targets = Vec::new();
    let mut app_targets = Vec::new();

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

        app_targets.push(target.clone());
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

    (facts, rules, app_targets)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let scale = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1000000);

    println!("Realistic Build System Usage Benchmark");
    println!("=======================================");
    println!("Scale: {} targets", scale);
    println!("\nScenario: User runs 'build //app:main'");
    println!("System must compute transitive deps of ONE target\n");

    let total_start = Instant::now();

    let (facts, rules, app_targets) = generate_realistic_graph(scale);
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

    let setup_time = total_start.elapsed();
    println!("Total setup: {:?}\n", setup_time);
    println!("--- Single Target Build (LAZY) ---");
    let target = &app_targets[app_targets.len() / 2]; // Middle app

    let query_start = Instant::now();
    let results = db.query("transitive_deps", &[Some(target), None]);
    let query_time = query_start.elapsed();

    println!("Query transitive_deps({}, _)", target);
    println!("  Time: {:?}", query_time);
    println!("  Results: {} transitive deps", results.len());
    println!("  Total (setup + query): {:?}", setup_time + query_time);

    println!("\n--- Full TC Computation (EAGER) ---");
    let mut db2 = Engine::new();
    db2.insert_facts(generate_realistic_graph(scale).0);
    for rule in generate_realistic_graph(scale).1 {
        db2.compile_rule(rule);
    }

    let eager_start = Instant::now();
    let all_results = db2.query("transitive_deps", &[]);
    let eager_time = eager_start.elapsed();

    println!("Query transitive_deps(_, _)");
    println!("  Time: {:?}", eager_time);
    println!("  Results: {} total pairs", all_results.len());

    println!("\n=== Summary ===");
    println!("For typical build system usage (building ONE target):");
    println!(
        "  Lazy:  {:?} ({}x faster)",
        setup_time + query_time,
        eager_time.as_secs_f64() / (setup_time + query_time).as_secs_f64()
    );
    println!("  Eager: {:?}", eager_time);
    println!("\nGoal: 1M targets in <10s total");
    if (setup_time + query_time).as_secs() < 10 {
        println!("✓ GOAL MET");
    } else {
        println!(
            "✗ Missed by {:?}",
            (setup_time + query_time) - std::time::Duration::from_secs(10)
        );
    }
}
