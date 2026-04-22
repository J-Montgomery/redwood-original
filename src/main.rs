use clap::Parser;
use redwood::cli::{handle_build, handle_query};
use redwood::format;
use redwood::runtime::prelude;
use std::fs;
use std::path::Path;

fn main() {
    let cli = Cli::parse();

    if let Some(backend) = &cli.backend {
        std::env::set_var("REDWOOD_BACKEND", backend);
    }

    let result = match cli.command {
        Some(Command::Query { query, dry_run }) => handle_query(&query, dry_run),
        Some(Command::Build {
            targets,
            with,
            dry_run,
            recursive,
        }) => handle_build(targets, with, dry_run, recursive),
        Some(Command::Format { path }) => handle_format(path),
        Some(Command::DumpPrelude) => handle_dump_prelude(),
        None => {
            eprintln!("No command specified. Use --help for usage information.");
            std::process::exit(1);
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn handle_format(path: Option<String>) -> Result<(), String> {
    let target_path = path.unwrap_or_else(|| ".".to_string());
    let path = Path::new(&target_path);

    let build_files = format::find_build_files(path)?;

    if build_files.is_empty() {
        println!("No BUILD.datalog files found");
        return Ok(());
    }

    let mut errors = Vec::new();
    let mut formatted_count = 0;

    for file_path in build_files {
        let path = Path::new(&file_path);
        match format::format_file(path) {
            Ok(formatted) => {
                fs::write(path, formatted)
                    .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;
                formatted_count += 1;
            }
            Err(e) => {
                errors.push(e);
            }
        }
    }

    if !errors.is_empty() {
        for error in errors {
            eprintln!("{}", error);
        }
        return Err("Some files had syntax errors".to_string());
    }

    println!("Formatted {} file(s)", formatted_count);
    Ok(())
}

fn handle_dump_prelude() -> Result<(), String> {
    let prelude_files = prelude::get_prelude_content();

    for (name, content) in prelude_files {
        println!("# {}", name);
        println!("{}", content);
    }

    Ok(())
}

#[derive(Parser)]
#[command(name = "redwood")]
#[command(about = "A Datalog-based build system", long_about = None)]
struct Cli {
    #[arg(long, value_name = "TYPE", help = "Backend to use: hashmap or dd")]
    backend: Option<String>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(clap::Subcommand)]
enum Command {
    Query {
        query: String,

        #[arg(long, help = "Show what would be built without executing")]
        dry_run: bool,
    },
    Build {
        targets: Vec<String>,

        #[arg(long, help = "Inject datalog (facts/rules) before build (repeatable)")]
        with: Vec<String>,

        #[arg(long, help = "Show what would be built without executing")]
        dry_run: bool,

        #[arg(long, short = 'r', help = "Recursively load BUILD.datalog files from workspace")]
        recursive: bool,
    },
    Format {
        path: Option<String>,
    },
    DumpPrelude,
}
