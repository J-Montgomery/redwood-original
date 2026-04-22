use redwood::datalog::Engine;
use redwood::datalog::{Fact, Predicate, Rule, Term, Value};
use std::time::Instant;

fn generate_layered_graph(num_targets: usize) -> (Vec<Fact>, Vec<Rule>) {
    let mut facts = Vec::new();
    let num_layers = (num_targets as f64).log2().ceil() as usize;
    let targets_per_layer = num_targets / num_layers;

    let mut all_targets: Vec<String> = Vec::new();

    for layer in 0..num_layers {
        let start_idx = layer * targets_per_layer;
        let end_idx = if layer == num_layers - 1 {
            num_targets
        } else {
            (layer + 1) * targets_per_layer
        };

        for i in start_idx..end_idx {
            let target = format!("//layer{}:target{}", layer, i);
            facts.push(Fact {
                predicate: "target".to_string(),
                args: vec![Value::String(target.clone())],
            });

            if layer > 0 {
                let fanin = 3.min(targets_per_layer);
                for j in 0..fanin {
                    let prev_layer_idx =
                        ((layer - 1) * targets_per_layer) + ((i * 7 + j * 11) % targets_per_layer);
                    if prev_layer_idx < all_targets.len() {
                        facts.push(Fact {
                            predicate: "deps".to_string(),
                            args: vec![
                                Value::String(target.clone()),
                                Value::String(all_targets[prev_layer_idx].clone()),
                            ],
                        });
                    }
                }
            }

            all_targets.push(target);
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

    (facts, rules)
}

fn benchmark_scale(num_targets: usize) {
    println!("\n=== {} targets ===", num_targets);

    let gen_start = Instant::now();
    let (facts, rules) = generate_layered_graph(num_targets);
    println!("Graph generation: {:?}", gen_start.elapsed());

    let num_direct_deps = facts.iter().filter(|f| f.predicate == "deps").count();
    println!("Facts: {} ({} direct deps)", facts.len(), num_direct_deps);

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
    println!("Compile rules: {:?}", compile_time);

    let query_start = Instant::now();
    let top_target = format!(
        "//layer{}:target{}",
        ((num_targets as f64).log2().ceil() as usize) - 1,
        num_targets - 1
    );
    let results = db.query("transitive_deps", &[Some(&top_target), None]);
    let query_time = query_start.elapsed();
    println!(
        "Query one target: {:?} ({} deps)",
        query_time,
        results.len()
    );

    let total = insert_time + compile_time + query_time;
    println!("Total: {:?}", total);

    let targets_per_sec = num_targets as f64 / total.as_secs_f64();
    println!("Throughput: {:.0} targets/sec", targets_per_sec);
}

fn main() {
    println!("Scalability Benchmark");
    println!("=====================");
    println!("Goal: 1M+ targets, sub-second queries\n");

    let scales = vec![1_000, 10_000, 100_000, 1_000_000];

    for scale in scales {
        benchmark_scale(scale);
    }

    println!("\n=== Summary ===");
    println!("Targets  | Insert   | Compile  | Query    | Total");
    println!("---------|----------|----------|----------|----------");
}
