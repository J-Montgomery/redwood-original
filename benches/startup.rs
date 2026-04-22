use redwood::datalog::parser;
use redwood::datalog::Engine;
use std::time::Instant;

fn generate_build_file(num_targets: usize) -> String {
    let mut content = String::new();

    for i in 0..num_targets {
        let package = i / 100;
        let target_name = format!("target{}", i);

        content.push_str(&format!("target(\"//pkg{}:{}\").\n", package, target_name));
        content.push_str(&format!(
            "kind(\"//pkg{}:{}\", rust_binary).\n",
            package, target_name
        ));
        content.push_str(&format!(
            "sources(\"//pkg{}:{}\", \"src/lib.rs\").\n",
            package, target_name
        ));

        if i > 0 {
            for j in 0..(3.min(i)) {
                let dep_idx = i - j - 1;
                let dep_package = dep_idx / 100;
                let dep_name = format!("target{}", dep_idx);
                content.push_str(&format!(
                    "deps(\"//pkg{}:{}\", \"//pkg{}:{}\").\n",
                    package, target_name, dep_package, dep_name
                ));
            }
        }

        content.push('\n');
    }

    content
}

fn benchmark_startup(num_targets: usize) {
    println!("\n=== Startup with {} targets ===", num_targets);

    let gen_start = Instant::now();
    let build_content = generate_build_file(num_targets);
    println!(
        "Generate BUILD file: {:?} ({} bytes)",
        gen_start.elapsed(),
        build_content.len()
    );

    let parse_start = Instant::now();
    let parse_result = parser::parse_program(&build_content);
    let parse_time = parse_start.elapsed();

    match parse_result {
        Ok((facts, _rules)) => {
            println!("Parse BUILD file: {:?} ({} facts)", parse_time, facts.len());

            let db_start = Instant::now();
            let mut db = Engine::new();
            db.insert_facts(facts);
            let db_time = db_start.elapsed();
            println!("Create database: {:?}", db_time);

            let query_start = Instant::now();
            let targets = db.query("target", &[]);
            let query_time = query_start.elapsed();
            println!(
                "Query targets: {:?} ({} results)",
                query_time,
                targets.len()
            );

            let total = parse_time + db_time + query_time;
            println!("Total startup: {:?}", total);

            if total.as_millis() < 1 {
                println!("✓ Sub-millisecond startup achieved");
            } else if total.as_millis() < 100 {
                println!("✓ Fast startup (<100ms)");
            } else {
                println!("✗ Slow startup");
            }
        }
        Err(e) => {
            println!("Parse error: {}", e);
        }
    }
}

fn main() {
    println!("Startup Time Benchmark");
    println!("======================");
    println!("Spec Goal: Sub-millisecond startup\n");

    let scales = vec![10, 100, 1_000, 10_000, 50_000];

    for scale in scales {
        benchmark_startup(scale);
    }

    println!("\n=== Analysis ===");
    println!("Startup time includes:");
    println!("1. Parse BUILD.datalog files");
    println!("2. Convert statements to facts");
    println!("3. Insert facts into database");
    println!("4. Execute initial queries");
}
