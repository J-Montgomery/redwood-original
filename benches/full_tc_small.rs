use redwood::datalog::Engine;
use redwood::datalog::{Fact, Predicate, Rule, Term, Value};
use std::time::Instant;

fn generate_graph(num_targets: usize) -> (Vec<Fact>, Vec<Rule>) {
    let mut facts = Vec::new();

    for i in 0..num_targets {
        let target = format!("//pkg:lib{}", i);
        facts.push(Fact {
            predicate: "target".to_string(),
            args: vec![Value::String(target.clone())],
        });

        if i > 0 {
            let fanout = 3.min(i);
            for j in 0..fanout {
                let dep_idx = i - j - 1;
                facts.push(Fact {
                    predicate: "deps".to_string(),
                    args: vec![
                        Value::String(target.clone()),
                        Value::String(format!("//pkg:lib{}", dep_idx)),
                    ],
                });
            }
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

fn benchmark_full_tc(num_targets: usize) {
    println!("\n=== {} targets ===", num_targets);

    let (facts, rules) = generate_graph(num_targets);
    let num_edges = facts.iter().filter(|f| f.predicate == "deps").count();
    println!("Graph: {} nodes, {} edges", num_targets, num_edges);

    let mut db = Engine::new();
    db.insert_facts(facts);
    for rule in rules {
        db.compile_rule(rule);
    }

    let start = Instant::now();
    let results = db.query("transitive_deps", &[]);
    let elapsed = start.elapsed();

    println!("Full TC: {:?} ({} pairs)", elapsed, results.len());
    println!(
        "Pairs/sec: {:.0}",
        results.len() as f64 / elapsed.as_secs_f64()
    );
}

fn main() {
    println!("Full Transitive Closure Benchmark");
    println!("==================================");
    println!("Limited to ≤100 targets (O(n³) complexity)\n");
    println!("Use case: Whole-graph analysis, not normal builds");
    println!("Normal builds use lazy TC (single target)\n");

    let scales = vec![10, 25, 50, 100];

    for scale in scales {
        benchmark_full_tc(scale);
    }

    println!("\n=== Analysis ===");
    println!("Full TC computes ALL transitive pairs.");
    println!("Complexity: O(n³) with semi-naive evaluation");
    println!("10 targets: ~50 pairs");
    println!("100 targets: ~5,000 pairs");
    println!("1,000 targets: ~500,000 pairs (too slow)");
    println!("\nFor normal builds, use lazy TC (query_patterns.rs)");
}
