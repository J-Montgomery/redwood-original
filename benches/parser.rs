use redwood::datalog::parser;
use std::time::Instant;

fn main() {
    println!("Parser Benchmark");
    println!("================\n");

    let sizes = [100, 1_000, 10_000, 50_000, 1_000_000];

    for &size in &sizes {
        let mut input = String::new();
        for i in 0..size {
            input.push_str(&format!("target(\"//target:t{}\").\n", i));
            input.push_str(&format!("kind(\"//target:t{}\", \"rust_binary\").\n", i));
            input.push_str(&format!(
                "deps(\"//target:t{}\", \"//target:t{}\").\n",
                i,
                (i + 1) % size
            ));
        }

        let start = Instant::now();
        let (_facts, _rules) = parser::parse_program(&input).unwrap();
        let elapsed = start.elapsed();

        println!(
            "{:>7} facts: {:>9?} ({:.2} µs/fact, {} bytes)",
            size * 3,
            elapsed,
            elapsed.as_micros() as f64 / (size * 3) as f64,
            input.len()
        );
    }

    println!("\n=== With rules ===");
    for &size in &sizes {
        let mut input = String::new();
        for i in 0..size {
            input.push_str(&format!("edge(\"n{}\", \"n{}\").\n", i, (i + 1) % size));
        }
        input.push_str("path(X, Y) :- edge(X, Y).\n");
        input.push_str("path(X, Z) :- edge(X, Y), path(Y, Z).\n");

        let start = Instant::now();
        let (_facts, rules) = parser::parse_program(&input).unwrap();
        let elapsed = start.elapsed();

        println!(
            "{:>6} items: {:>8?} ({} facts, {} rules)",
            size + 2,
            elapsed,
            size,
            rules.len()
        );
    }
}
