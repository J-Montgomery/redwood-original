use redwood::datalog::{Fact, Value};
use std::time::Instant;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use xxhash_rust::xxh3::xxh3_64;

fn generate_layered_graph(num_targets: usize) -> Vec<Fact> {
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

    facts
}

fn main() {
    println!("TC Detailed Breakdown");
    println!("=====================\n");

    let scale = 1_000_000;
    println!("Scale: {} targets\n", scale);

    let deps_facts = generate_layered_graph(scale);
    println!("Generated {} edge facts", deps_facts.len());

    let step1_start = Instant::now();
    let mut string_to_id: HashMap<u64, u32> = HashMap::with_capacity(deps_facts.len());
    let mut id_to_fact_ref: Vec<(usize, u8)> = Vec::with_capacity(deps_facts.len());

    for (fact_idx, fact) in deps_facts.iter().enumerate() {
        if let (Value::String(src), Value::String(dst)) = (&fact.args[0], &fact.args[1]) {
            let src_hash = xxh3_64(src.as_bytes());
            if !string_to_id.contains_key(&src_hash) {
                string_to_id.insert(src_hash, id_to_fact_ref.len() as u32);
                id_to_fact_ref.push((fact_idx, 0));
            }
            let dst_hash = xxh3_64(dst.as_bytes());
            if !string_to_id.contains_key(&dst_hash) {
                string_to_id.insert(dst_hash, id_to_fact_ref.len() as u32);
                id_to_fact_ref.push((fact_idx, 1));
            }
        }
    }
    let step1_time = step1_start.elapsed();
    println!("Step 1 - Build hash->id mapping (zero-copy): {:?} ({} unique nodes)", step1_time, id_to_fact_ref.len());

    let step2_start = Instant::now();
    let mut adj_list: Vec<Vec<u32>> = vec![Vec::new(); id_to_fact_ref.len()];
    for fact in &deps_facts {
        if let (Value::String(src), Value::String(dst)) = (&fact.args[0], &fact.args[1]) {
            let src_hash = xxh3_64(src.as_bytes());
            let dst_hash = xxh3_64(dst.as_bytes());
            let src_id = string_to_id[&src_hash];
            let dst_id = string_to_id[&dst_hash];
            adj_list[src_id as usize].push(dst_id);
        }
    }
    let step2_time = step2_start.elapsed();
    println!("Step 2 - Build adjacency list: {:?}", step2_time);

    let top_target = format!(
        "//layer{}:target{}",
        ((scale as f64).log2().ceil() as usize) - 1,
        scale - 1
    );
    let top_target_hash = xxh3_64(top_target.as_bytes());
    let start_id = string_to_id[&top_target_hash];

    let step3_start = Instant::now();
    let mut visited: Vec<bool> = vec![false; id_to_fact_ref.len()];
    let mut queue: VecDeque<u32> = VecDeque::new();
    queue.push_back(start_id);
    visited[start_id as usize] = true;

    let mut reachable_ids: Vec<u32> = Vec::new();

    while let Some(node) = queue.pop_front() {
        for &neighbor in &adj_list[node as usize] {
            if !visited[neighbor as usize] {
                visited[neighbor as usize] = true;
                reachable_ids.push(neighbor);
                queue.push_back(neighbor);
            }
        }
    }
    let step3_time = step3_start.elapsed();
    println!("Step 3 - BFS traversal: {:?} ({} reachable)", step3_time, reachable_ids.len());

    let step4_start = Instant::now();
    let head_name = "transitive_deps".to_string();
    let start_value = Value::String(top_target.clone());
    let mut results: Vec<Rc<Fact>> = Vec::with_capacity(reachable_ids.len());

    for id in &reachable_ids {
        let (fact_idx, arg_pos) = id_to_fact_ref[*id as usize];
        if let Value::String(dst_string) = &deps_facts[fact_idx].args[arg_pos as usize] {
            results.push(Rc::new(Fact {
                predicate: head_name.clone(),
                args: vec![start_value.clone(), Value::String(dst_string.clone())],
            }));
        }
    }
    let step4_time = step4_start.elapsed();
    println!("Step 4 - Build result Facts: {:?}", step4_time);

    let total = step1_time + step2_time + step3_time + step4_time;
    println!("\nTotal: {:?}", total);
    println!("\nBreakdown:");
    println!("  String mapping: {:?} ({:.1}%)", step1_time, step1_time.as_secs_f64() / total.as_secs_f64() * 100.0);
    println!("  Adjacency list: {:?} ({:.1}%)", step2_time, step2_time.as_secs_f64() / total.as_secs_f64() * 100.0);
    println!("  BFS traversal:  {:?} ({:.1}%)", step3_time, step3_time.as_secs_f64() / total.as_secs_f64() * 100.0);
    println!("  Result Facts:   {:?} ({:.1}%)", step4_time, step4_time.as_secs_f64() / total.as_secs_f64() * 100.0);
}
