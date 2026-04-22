use redwood::datalog::{Fact, Value};
use std::time::Instant;
use std::collections::{HashSet, VecDeque, HashMap};

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
    println!("BFS Variants Comparison");
    println!("=======================\n");

    let scale = 1_000_000;
    println!("Scale: {} targets\n", scale);

    let deps_facts = generate_layered_graph(scale);
    println!("Generated {} edge facts", deps_facts.len());

    // Build string->index mapping for integer-based BFS
    let mut string_to_id: HashMap<String, u32> = HashMap::new();
    let mut id_to_string: Vec<String> = Vec::new();

    for fact in &deps_facts {
        if let (Value::String(src), Value::String(dst)) = (&fact.args[0], &fact.args[1]) {
            if !string_to_id.contains_key(src) {
                string_to_id.insert(src.clone(), id_to_string.len() as u32);
                id_to_string.push(src.clone());
            }
            if !string_to_id.contains_key(dst) {
                string_to_id.insert(dst.clone(), id_to_string.len() as u32);
                id_to_string.push(dst.clone());
            }
        }
    }
    println!("Unique nodes: {}", id_to_string.len());

    // Build adjacency list with integer IDs
    let mut adj_list: Vec<Vec<u32>> = vec![Vec::new(); id_to_string.len()];
    for fact in &deps_facts {
        if let (Value::String(src), Value::String(dst)) = (&fact.args[0], &fact.args[1]) {
            let src_id = string_to_id[src];
            let dst_id = string_to_id[dst];
            adj_list[src_id as usize].push(dst_id);
        }
    }

    // Build string-based index
    let mut string_index: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, fact) in deps_facts.iter().enumerate() {
        if let Value::String(src) = &fact.args[0] {
            string_index.entry(src.clone()).or_default().push(i);
        }
    }

    let top_target = format!(
        "//layer{}:target{}",
        ((scale as f64).log2().ceil() as usize) - 1,
        scale - 1
    );
    let start_id = string_to_id[&top_target];
    let start_value = Value::String(top_target.clone());

    // Variant 1: Current implementation (HashSet<&Value>)
    let start = Instant::now();
    let mut visited1: HashSet<&Value> = HashSet::new();
    let mut queue1: VecDeque<&Value> = VecDeque::new();
    queue1.push_back(&start_value);
    visited1.insert(&start_value);

    while let Some(node) = queue1.pop_front() {
        if let Value::String(node_str) = node {
            if let Some(indices) = string_index.get(node_str) {
                for &idx in indices {
                    let neighbor = &deps_facts[idx].args[1];
                    if visited1.insert(neighbor) {
                        queue1.push_back(neighbor);
                    }
                }
            }
        }
    }
    let time1 = start.elapsed();
    println!("\n1. HashSet<&Value> + string index: {:?} ({} nodes)", time1, visited1.len());

    // Variant 2: Integer IDs with HashSet<u32>
    let start = Instant::now();
    let mut visited2: HashSet<u32> = HashSet::new();
    let mut queue2: VecDeque<u32> = VecDeque::new();
    queue2.push_back(start_id);
    visited2.insert(start_id);

    while let Some(node) = queue2.pop_front() {
        for &neighbor in &adj_list[node as usize] {
            if visited2.insert(neighbor) {
                queue2.push_back(neighbor);
            }
        }
    }
    let time2 = start.elapsed();
    println!("2. HashSet<u32> + adjacency list:  {:?} ({} nodes)", time2, visited2.len());

    // Variant 3: Bit vector for visited (fastest possible)
    let start = Instant::now();
    let mut visited3: Vec<bool> = vec![false; id_to_string.len()];
    let mut queue3: VecDeque<u32> = VecDeque::new();
    queue3.push_back(start_id);
    visited3[start_id as usize] = true;
    let mut count3 = 1usize;

    while let Some(node) = queue3.pop_front() {
        for &neighbor in &adj_list[node as usize] {
            if !visited3[neighbor as usize] {
                visited3[neighbor as usize] = true;
                count3 += 1;
                queue3.push_back(neighbor);
            }
        }
    }
    let time3 = start.elapsed();
    println!("3. Vec<bool> + adjacency list:     {:?} ({} nodes)", time3, count3);

    // Variant 4: Pre-sized HashSet with capacity
    let start = Instant::now();
    let mut visited4: HashSet<u32> = HashSet::with_capacity(500_000);
    let mut queue4: VecDeque<u32> = VecDeque::with_capacity(100_000);
    queue4.push_back(start_id);
    visited4.insert(start_id);

    while let Some(node) = queue4.pop_front() {
        for &neighbor in &adj_list[node as usize] {
            if visited4.insert(neighbor) {
                queue4.push_back(neighbor);
            }
        }
    }
    let time4 = start.elapsed();
    println!("4. HashSet<u32> pre-sized:         {:?} ({} nodes)", time4, visited4.len());

    println!("\nSpeedup vs current:");
    println!("  Integer IDs: {:.1}x faster", time1.as_secs_f64() / time2.as_secs_f64());
    println!("  Bit vector:  {:.1}x faster", time1.as_secs_f64() / time3.as_secs_f64());
}
