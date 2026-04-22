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

fn main() {
    println!("TC New Implementation Benchmark");
    println!("================================\n");

    for scale in [100_000, 1_000_000] {
        println!("=== {} targets ===", scale);

        let (facts, rules) = generate_layered_graph(scale);
        let num_deps = facts.iter().filter(|f| f.predicate == "deps").count();
        println!("Facts: {} ({} deps)", facts.len(), num_deps);

        let mut db = Engine::new();
        db.insert_facts(facts);
        for rule in rules {
            db.compile_rule(rule);
        }

        let top_target = format!(
            "//layer{}:target{}",
            ((scale as f64).log2().ceil() as usize) - 1,
            scale - 1
        );

        // First query (cold - includes adjacency list build)
        let start = Instant::now();
        let results = db.query("transitive_deps", &[Some(&top_target), None]);
        let cold_time = start.elapsed();
        println!("Query 1 (cold): {:?} ({} results)", cold_time, results.len());

        // Second query (warm - adjacency list cached)
        let start = Instant::now();
        let results = db.query("transitive_deps", &[Some(&top_target), None]);
        let warm_time = start.elapsed();
        println!("Query 2 (warm): {:?} ({} results)", warm_time, results.len());

        // Third query - different target
        let mid_target = format!(
            "//layer{}:target{}",
            ((scale as f64).log2().ceil() as usize) / 2,
            scale / 2
        );
        let start = Instant::now();
        let results = db.query("transitive_deps", &[Some(&mid_target), None]);
        let mid_time = start.elapsed();
        println!("Query 3 (mid):  {:?} ({} results)", mid_time, results.len());

        println!();
    }
}
