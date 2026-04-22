use redwood::datalog::Engine;
use redwood::datalog::{Fact, Predicate, Rule, Term, Value};
use std::time::Instant;
use std::collections::{HashSet, VecDeque, HashMap};
use std::rc::Rc;

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
    println!("TC Breakdown: BFS vs Allocation");
    println!("================================\n");

    for scale in [100_000, 1_000_000] {
        println!("=== {} targets ===\n", scale);

        let (facts, _rules) = generate_layered_graph(scale);

        // Build index manually for direct measurement
        let deps_facts: Vec<_> = facts.iter()
            .filter(|f| f.predicate == "deps")
            .map(|f| f.clone())
            .collect();

        let mut index: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, fact) in deps_facts.iter().enumerate() {
            if let Value::String(src) = &fact.args[0] {
                index.entry(src.clone()).or_default().push(i);
            }
        }

        let top_target = format!(
            "//layer{}:target{}",
            ((scale as f64).log2().ceil() as usize) - 1,
            scale - 1
        );
        let start_value = Value::String(top_target.clone());

        println!("Deps facts: {}", deps_facts.len());
        println!("Index entries: {}", index.len());

        // Measure BFS only (no allocation)
        let bfs_start = Instant::now();
        let mut visited: HashSet<&Value> = HashSet::new();
        let mut queue: VecDeque<&Value> = VecDeque::new();
        queue.push_back(&start_value);
        visited.insert(&start_value);

        while let Some(node) = queue.pop_front() {
            if let Value::String(node_str) = node {
                if let Some(indices) = index.get(node_str) {
                    for &idx in indices {
                        let fact = &deps_facts[idx];
                        if fact.args.len() == 2 {
                            let neighbor = &fact.args[1];
                            if visited.insert(neighbor) {
                                queue.push_back(neighbor);
                            }
                        }
                    }
                }
            }
        }
        let bfs_time = bfs_start.elapsed();
        let num_reachable = visited.len() - 1;
        println!("BFS traversal: {:?} ({} reachable nodes)", bfs_time, num_reachable);

        // Measure allocation only (Facts with strings)
        let alloc_start = Instant::now();
        let head_name = "transitive_deps".to_string();
        let start_clone = start_value.clone();
        let mut results: Vec<Rc<Fact>> = Vec::with_capacity(num_reachable);

        for dst in visited.iter().skip(1) {  // skip start node
            results.push(Rc::new(Fact {
                predicate: head_name.clone(),
                args: vec![start_clone.clone(), (*dst).clone()],
            }));
        }
        let alloc_time = alloc_start.elapsed();
        println!("Fact allocation: {:?} ({} facts)", alloc_time, results.len());

        // Measure allocation with pre-interned predicate (single Rc<str>)
        let alloc2_start = Instant::now();
        let head_rc: Rc<str> = Rc::from("transitive_deps");
        let mut results2: Vec<(Rc<str>, Value, Value)> = Vec::with_capacity(num_reachable);

        for dst in visited.iter().skip(1) {
            results2.push((Rc::clone(&head_rc), start_clone.clone(), (*dst).clone()));
        }
        let alloc2_time = alloc2_start.elapsed();
        println!("Tuple allocation (Rc<str>): {:?}", alloc2_time);

        // Measure just counting (no allocation at all)
        let count_start = Instant::now();
        let count = visited.len() - 1;
        let count_time = count_start.elapsed();
        println!("Just count: {:?} ({} results)", count_time, count);

        // Total as comparison
        let total = bfs_time + alloc_time;
        println!("\nBreakdown:");
        println!("  BFS:        {:?} ({:.1}%)", bfs_time, bfs_time.as_secs_f64() / total.as_secs_f64() * 100.0);
        println!("  Allocation: {:?} ({:.1}%)", alloc_time, alloc_time.as_secs_f64() / total.as_secs_f64() * 100.0);
        println!();
    }
}
