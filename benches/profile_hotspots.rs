#![allow(unused)]
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
    println!("Profiling Hotspots");
    println!("==================\n");

    let num_targets = 100_000;
    println!("Testing with {} targets\n", num_targets);

    let (facts, rules) = generate_layered_graph(num_targets);
    let num_deps = facts.iter().filter(|f| f.predicate == "deps").count();
    println!("Generated {} facts ({} deps)\n", facts.len(), num_deps);

    let mut db = Engine::new();

    // Profile insert
    let start = Instant::now();
    db.insert_facts(facts);
    println!("Insert: {:?}", start.elapsed());

    // Profile compile
    let start = Instant::now();
    for rule in rules {
        db.compile_rule(rule);
    }
    println!("Compile: {:?}", start.elapsed());

    // Profile multiple queries to warm cache and measure steady state
    let top_target = format!(
        "//layer{}:target{}",
        ((num_targets as f64).log2().ceil() as usize) - 1,
        num_targets - 1
    );

    println!("\nQuery profiling (target: {}):", top_target);

    // First query (cold)
    let start = Instant::now();
    let results = db.query("transitive_deps", &[Some(&top_target), None]);
    println!("  Query 1 (cold): {:?} ({} results)", start.elapsed(), results.len());

    // Second query (should hit cache)
    let start = Instant::now();
    let results = db.query("transitive_deps", &[Some(&top_target), None]);
    println!("  Query 2 (warm): {:?} ({} results)", start.elapsed(), results.len());

    // Query different target
    let mid_target = format!(
        "//layer{}:target{}",
        ((num_targets as f64).log2().ceil() as usize) / 2,
        num_targets / 2
    );
    let start = Instant::now();
    let results = db.query("transitive_deps", &[Some(&mid_target), None]);
    println!("  Query 3 (mid, cold): {:?} ({} results)", start.elapsed(), results.len());

    // Multiple small queries
    println!("\n10 small queries:");
    let start = Instant::now();
    for i in 0..10 {
        let target = format!("//layer1:target{}", i * 100);
        let _ = db.query("transitive_deps", &[Some(&target), None]);
    }
    println!("  Total: {:?}", start.elapsed());

    // Profile base fact queries
    println!("\nBase fact queries:");
    let start = Instant::now();
    for _ in 0..100 {
        let _ = db.query("deps", &[Some(&top_target), None]);
    }
    println!("  100x deps query: {:?}", start.elapsed());

    let start = Instant::now();
    for _ in 0..100 {
        let _ = db.query("target", &[]);
    }
    println!("  100x target query (all): {:?}", start.elapsed());
}
