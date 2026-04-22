use redwood::datalog::Engine;
use redwood::datalog::{Fact, Value};
use std::time::Instant;

fn generate_target_facts(start: usize, count: usize) -> Vec<Fact> {
    let mut facts = Vec::new();
    for i in start..(start + count) {
        let target = format!("//target:t{}", i);
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
        for j in 0..3 {
            facts.push(Fact {
                predicate: "deps".to_string(),
                args: vec![
                    Value::String(target.clone()),
                    Value::String(format!("//target:t{}", (i + j + 1) % 1_000_000)),
                ],
            });
        }
    }
    facts
}

fn main() {
    let num_targets = 100_000;

    println!("Daemon Mode Benchmark");
    println!("=====================");
    println!("Simulates long-running daemon with incremental updates\n");

    let mut db = Engine::new();

    let facts = generate_target_facts(0, num_targets);
    let start = Instant::now();
    db.insert_facts(facts);
    let cold_start = start.elapsed();
    println!("Cold start ({} targets): {:?}", num_targets, cold_start);

    let update_sizes = [1, 10, 100, 1_000];

    println!("\n=== Incremental Updates ===");
    for &size in &update_sizes {
        let old_facts = generate_target_facts(0, size);
        let start = Instant::now();
        db.retract_facts(old_facts);
        let retract_time = start.elapsed();

        let new_facts = generate_target_facts(num_targets, size);
        let start = Instant::now();
        db.insert_facts(new_facts);
        let insert_time = start.elapsed();

        let total_update = retract_time + insert_time;

        println!(
            "{:>4} files: retract {:>8?}, insert {:>8?}, total {:>8?}",
            size, retract_time, insert_time, total_update
        );
    }

    println!("\n=== Full Rebuild ===");
    let all_facts = generate_target_facts(0, num_targets);
    let start = Instant::now();
    db.retract_facts(all_facts);
    let full_retract = start.elapsed();

    let new_all = generate_target_facts(0, num_targets);
    let start = Instant::now();
    db.insert_facts(new_all);
    let full_insert = start.elapsed();

    let full_rebuild = full_retract + full_insert;
    println!("Retract all: {:?}", full_retract);
    println!("Insert all:  {:?}", full_insert);
    println!(
        "Total:       {:?} ({:.2}x cold start)",
        full_rebuild,
        full_rebuild.as_secs_f64() / cold_start.as_secs_f64()
    );
}
