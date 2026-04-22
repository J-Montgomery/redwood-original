use crate::build::graph_generator::GraphGenerator;
use crate::build::system_tool::SystemTool;
use crate::build::{BuildKindRegistry, BuildPlan, Executor};
use crate::cache;
use crate::datalog::{parser, Engine, Fact, TargetLabel, Value};
use crate::runtime::{prelude, ToolchainScanner};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

struct NamespaceLoader {
    loaded: HashSet<String>,
    roots: HashMap<String, PathBuf>,
}

impl NamespaceLoader {
    fn new() -> Self {
        let mut roots = HashMap::new();
        roots.insert("//".to_string(), PathBuf::from("."));
        Self {
            loaded: HashSet::new(),
            roots,
        }
    }

    fn register_root(&mut self, namespace: String, path: PathBuf) {
        self.roots.insert(namespace, path);
    }

    fn load_namespace(&mut self, namespace: &str, engine: &mut Engine) -> Result<(), String> {
        if self.loaded.contains(namespace) {
            return Ok(());
        }

        let root = self.roots.get(namespace).ok_or_else(|| {
            format!(
                "Unknown namespace: {}. Did you forget a root() fact?",
                namespace
            )
        })?;

        let file_path = root.join("BUILD.datalog");

        if !file_path.exists() {
            return Err(format!(
                "BUILD.datalog not found for namespace {} at {}",
                namespace,
                file_path.display()
            ));
        }

        let content = std::fs::read_to_string(&file_path)
            .map_err(|e| format!("Failed to read {}: {}", file_path.display(), e))?;

        let (facts, rules, locations) = parser::parse_program_with_namespace(
            &content,
            &file_path.to_string_lossy(),
            namespace,
        )?;

        for (key, loc) in locations {
            engine.record_source_location(&key, loc);
        }

        engine.insert_facts(facts);
        for rule in rules {
            engine.compile_rule(rule);
        }

        self.loaded.insert(namespace.to_string());
        Ok(())
    }
}

/// Recursively discover all BUILD.datalog files from the workspace root.
/// Returns a list of (package_path, file_path) pairs.
/// For example: ("//src/datalog", "src/datalog/BUILD.datalog")
fn discover_build_files(root: &Path) -> Vec<(String, PathBuf)> {
    let mut results = Vec::new();
    discover_build_files_recursive(root, root, &mut results);
    results
}

fn discover_build_files_recursive(root: &Path, dir: &Path, results: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Skip hidden directories and common non-source directories
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
        }

        if path.is_dir() {
            // Check for BUILD.datalog in this directory
            let build_file = path.join("BUILD.datalog");
            if build_file.exists() {
                // Compute package path relative to root
                if let Ok(rel_path) = path.strip_prefix(root) {
                    let package = if rel_path.as_os_str().is_empty() {
                        "//".to_string()
                    } else {
                        format!("//{}", rel_path.display())
                    };
                    results.push((package, build_file));
                }
            }
            // Recurse into subdirectory
            discover_build_files_recursive(root, &path, results);
        }
    }
}

/// Load a BUILD.datalog file with package-relative target namespacing.
/// Targets declared as "//pkg:target" become "//{package}/pkg:target"
fn load_package_build_file(
    package: &str,
    file_path: &Path,
    engine: &mut Engine,
) -> Result<(), String> {
    let content = std::fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read {}: {}", file_path.display(), e))?;

    // Parse with package prefix for target namespacing
    let (facts, rules, locations) = parser::parse_program_with_namespace(
        &content,
        &file_path.to_string_lossy(),
        package,
    )?;

    for (key, loc) in locations {
        engine.record_source_location(&key, loc);
    }

    engine.insert_facts(facts);
    for rule in rules {
        engine.compile_rule(rule);
    }

    Ok(())
}

fn extract_namespace(target_label: &str) -> String {
    let double_slash_count = target_label.matches("//").count();
    if double_slash_count > 1 {
        let parts: Vec<_> = target_label.split("//").collect();
        if parts.len() >= 2 {
            format!("//{}", parts[1])
        } else {
            "//".to_string()
        }
    } else {
        "//".to_string()
    }
}

fn expand_target_patterns(
    patterns: Vec<String>,
    db: &mut Engine,
    loader: &mut NamespaceLoader,
) -> Result<Vec<String>, String> {
    let mut expanded = Vec::new();

    for pattern in patterns {
        if pattern.ends_with("/...") {
            // Load all namespaces first (query for root() facts)
            let roots = db.query("root", &[]);
            for fact in roots {
                if !fact.args.is_empty() {
                    if let Value::String(namespace) = &fact.args[0] {
                        loader.load_namespace(namespace, db)?;
                    }
                }
            }

            // Strip "/..." to get prefix
            let prefix = pattern.strip_suffix("/...").unwrap();

            // Query matches_pattern(Target, Prefix)
            let results = db.query("matches_pattern", &[None, Some(prefix)]);
            for fact in results {
                if let Some(Value::String(target)) = fact.args.first() {
                    expanded.push(target.clone());
                }
            }
        } else {
            expanded.push(pattern);
        }
    }

    Ok(expanded)
}

fn parse_query_with_repl_syntax(
    query: &str,
) -> Result<(Vec<Fact>, Vec<crate::datalog::Rule>), String> {
    // First, try parsing as query body (preserves Variable/Constant distinction)
    // This handles queries like: source_location("//redwood:redwood", F, L)
    if !query.contains(":-") {
        // Add a trailing dot if not present for parse_query_body
        let query_for_body = if query.trim().ends_with('.') {
            query.to_string()
        } else {
            format!("{}.", query.trim())
        };

        if let Ok(body) = parser::parse_query_body(&query_for_body) {
            // Check if there are any variables in the query
            let mut variables = Vec::new();
            let mut seen = std::collections::HashSet::new();
            for pred in &body {
                for arg in &pred.args {
                    if let crate::datalog::Term::Variable(v) = arg {
                        if !v.starts_with("_anon_") && !seen.contains(v) {
                            variables.push(v.clone());
                            seen.insert(v.clone());
                        }
                    }
                }
            }

            // If there are variables, convert to a rule
            if !variables.is_empty() {
                let head_args: Vec<crate::datalog::Term> = variables
                    .into_iter()
                    .map(crate::datalog::Term::Variable)
                    .collect();

                let head = crate::datalog::Predicate {
                    name: "query".to_string(),
                    args: head_args,
                };

                let rule = crate::datalog::Rule::new(head, body);
                return Ok((vec![], vec![rule]));
            }
        }
    }

    // Try parsing as complete program (handles facts with trailing .)
    if let Ok(result) = parser::parse_program(query) {
        return Ok(result);
    }

    // Try adding trailing dot if missing
    let query_with_dot = if query.trim().ends_with('.') {
        query.to_string()
    } else {
        format!("{}.", query.trim())
    };

    if let Ok(result) = parser::parse_program(&query_with_dot) {
        return Ok(result);
    }

    Err("Failed to parse query".to_string())
}

pub fn handle_query(query: &str, _dry_run: bool) -> Result<(), String> {
    let mut db = Engine::new();

    // Set resolve callback to enable resolve() builtin
    db.set_resolve_callback(Box::new(|_target, tool, args| {
        let output = std::process::Command::new(tool)
            .args(args)
            .output()
            .map_err(|e| e.to_string())?;

        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }));

    if Path::new("BUILD.datalog").exists() {
        let content = std::fs::read_to_string("BUILD.datalog")
            .map_err(|e| format!("Failed to read BUILD.datalog: {}", e))?;

        let (facts, rules, locations) = parser::parse_program_with_file(&content, "BUILD.datalog")?;

        for (key, loc) in locations {
            db.record_source_location(&key, loc);
        }

        db.insert_facts(facts);

        for rule in rules {
            db.compile_rule(rule);
        }
    }

    let scanner = ToolchainScanner::new();
    let toolchain_facts = scanner.scan();
    db.insert_facts(toolchain_facts);

    let (prelude_facts, prelude_rules, prelude_locations) = prelude::get_prelude_with_locations();

    for (key, loc) in prelude_locations {
        db.record_source_location(&key, loc);
    }

    db.insert_facts(prelude_facts);

    for rule in prelude_rules {
        db.compile_rule(rule);
    }

    let file_facts = generate_file_exists_facts();
    db.insert_facts(file_facts);

    let (query_facts, rules) = parse_query_with_repl_syntax(query)?;

    let predicate = if let Some(rule) = rules.last() {
        rule.head.name.clone()
    } else if let Some(fact) = query_facts.last() {
        fact.predicate.clone()
    } else {
        return Err("No predicate found in query".to_string());
    };

    for fact in query_facts {
        // Skip facts that look like variable patterns (should have been converted to rules)
        if fact.args.iter().any(|arg| matches!(arg, Value::String(s) if s.chars().next().is_some_and(|c| c.is_uppercase()))) {
            continue;
        }
        db.insert_facts(vec![fact]);
    }

    for rule in rules {
        db.compile_rule(rule);
    }

    let results = db.query(&predicate, &[]);

    if predicate == "source_location" {
        for fact in results {
            if let [Value::String(pred), Value::String(file), Value::Integer(line)] = &fact.args[..]
            {
                println!("{}:{} - {}", file, line, pred);
            }
        }
    } else {
        for fact in results {
            print!("{}(", fact.predicate);
            for (i, arg) in fact.args.iter().enumerate() {
                if i > 0 {
                    print!(", ");
                }
                match arg {
                    Value::String(s) => print!("\"{}\"", s),
                    Value::Integer(n) => print!("{}", n),
                    Value::Bool(b) => print!("{}", b),
                    Value::Label(l) => print!("\"{}\"", l),
                    Value::Path(p) => print!("\"{}\"", p.display()),
                }
            }
            println!(").");
        }
    }

    Ok(())
}

pub fn handle_build(targets: Vec<String>, with: Vec<String>, dry_run: bool, recursive: bool) -> Result<(), String> {
    let total_start = std::time::Instant::now();

    let mut phase_start = std::time::Instant::now();
    let mut db = Engine::new();

    // Set resolve callback to enable resolve() builtin
    db.set_resolve_callback(Box::new(|_target, tool, args| {
        let output = std::process::Command::new(tool)
            .args(args)
            .output()
            .map_err(|e| e.to_string())?;

        if output.status.success() {
            Ok(output.stdout)
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }));
    let init_time = phase_start.elapsed();

    phase_start = std::time::Instant::now();
    let mut loader = NamespaceLoader::new();

    if Path::new("BUILD.datalog").exists() {
        let content = std::fs::read_to_string("BUILD.datalog")
            .map_err(|e| format!("Failed to read BUILD.datalog: {}", e))?;

        let (facts, rules, locations) = parser::parse_program_with_file(&content, "BUILD.datalog")?;

        // Extract root() facts to register namespaces
        for fact in &facts {
            if fact.predicate == "root" && fact.args.len() == 2 {
                if let (Value::String(namespace), Value::String(path_str)) =
                    (&fact.args[0], &fact.args[1])
                {
                    loader.register_root(namespace.clone(), PathBuf::from(path_str));
                }
            }
        }

        for (key, loc) in locations {
            db.record_source_location(&key, loc);
        }

        db.insert_facts(facts);

        for rule in rules {
            db.compile_rule(rule);
        }
    } else {
        return Err("BUILD.datalog not found".to_string());
    }

    // Recursively load BUILD.datalog files from subdirectories
    if recursive {
        let workspace_root = Path::new(".");
        let build_files = discover_build_files(workspace_root);

        for (package, file_path) in build_files {
            // Skip root BUILD.datalog (already loaded above)
            if package == "//" {
                continue;
            }
            load_package_build_file(&package, &file_path, &mut db)?;
        }
    }

    let parse_time = phase_start.elapsed();

    phase_start = std::time::Instant::now();
    let scanner = ToolchainScanner::new();
    let toolchain_facts = scanner.scan();
    db.insert_facts(toolchain_facts);
    let toolchain_time = phase_start.elapsed();

    phase_start = std::time::Instant::now();
    let (prelude_facts, prelude_rules, prelude_locations) = prelude::get_prelude_with_locations();

    for (key, loc) in prelude_locations {
        db.record_source_location(&key, loc);
    }

    db.insert_facts(prelude_facts);

    for rule in prelude_rules {
        db.compile_rule(rule);
    }
    let prelude_time = phase_start.elapsed();

    phase_start = std::time::Instant::now();
    for datalog in with {
        let (facts, rules) = parse_query_with_repl_syntax(&datalog)?;
        for fact in facts {
            // Check for variables in facts (uppercase, anonymous, or wildcard)
            let has_variables = fact.args.iter().any(|arg| {
                if let Value::String(s) = arg {
                    s == "_"
                        || s.chars().next().is_some_and(|c| c.is_uppercase())
                        || s.starts_with("_anon_")
                } else {
                    false
                }
            });

            if has_variables {
                eprintln!(
                    "Warning: Facts cannot contain variables: {}({}).",
                    fact.predicate,
                    fact.args
                        .iter()
                        .map(|arg| match arg {
                            Value::String(s) => format!("\"{}\"", s),
                            Value::Integer(i) => i.to_string(),
                            Value::Bool(b) => b.to_string(),
                            Value::Label(l) => format!("\"{}\"", l),
                            Value::Path(p) => format!("\"{}\"", p.display()),
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                eprintln!(
                    "         Did you mean: {}(X) :- target(X). ?",
                    fact.predicate
                );
                eprintln!("         (Use a rule to quantify over all targets)");
                continue;
            }
            db.insert_facts(vec![fact]);
        }
        for rule in rules {
            db.compile_rule(rule);
        }
    }
    let with_inject_time = phase_start.elapsed();

    phase_start = std::time::Instant::now();
    let file_facts = generate_file_exists_facts();
    db.insert_facts(file_facts);
    let file_scan_time = phase_start.elapsed();

    let mut registry = BuildKindRegistry::new();
    registry.register(Box::new(SystemTool::new()));
    registry.register(Box::new(GraphGenerator));
    registry.register(Box::new(
        crate::build::external_dependency::ExternalDependency::new(),
    ));

    let executor = Executor::new();

    phase_start = std::time::Instant::now();

    // Expand patterns like "//..." before parsing
    let expanded_targets = expand_target_patterns(targets, &mut db, &mut loader)?;

    let parsed_targets: Result<Vec<TargetLabel>, String> = expanded_targets
        .iter()
        .map(|s| TargetLabel::parse(s))
        .collect();
    let parsed_targets = parsed_targets?;

    let ordered_targets = topological_sort(&parsed_targets, &mut db, &mut loader)?;
    let build_levels = compute_build_levels(&ordered_targets, &mut db);
    let topo_time = phase_start.elapsed();

    let mut total_planning_time = std::time::Duration::ZERO;
    let mut total_execution_time = std::time::Duration::ZERO;

    for level in build_levels {
        let planning_start = std::time::Instant::now();
        let mut targets_to_build = Vec::new();

        for target in &level {
            let target_str = target.to_string();

            // Use filtered queries instead of querying all facts
            let outputs_facts = db.query("outputs", &[Some(&target_str)]);
            let target_outputs: Vec<String> = outputs_facts
                .iter()
                .filter_map(|f| {
                    if f.args.len() >= 2 {
                        if let Value::String(output) = &f.args[1] {
                            return Some(output.clone());
                        }
                    }
                    None
                })
                .collect();

            let mut output_exists_facts = Vec::new();
            for output in target_outputs {
                if Path::new(&output).exists() {
                    output_exists_facts.push(crate::datalog::Fact {
                        predicate: "file_exists".to_string(),
                        args: vec![Value::String(output)],
                    });
                }
            }
            db.insert_facts(output_exists_facts);

            let sources_facts = db.query("sources", &[Some(&target_str)]);
            let source_paths: Vec<String> = sources_facts
                .iter()
                .filter_map(|f| {
                    if f.args.len() >= 2 {
                        if let Value::String(source) = &f.args[1] {
                            return Some(source.clone());
                        }
                    }
                    None
                })
                .collect();

            let build_input_facts = db.query("build_input", &[Some(&target_str)]);
            let build_input_paths: Vec<String> = build_input_facts
                .iter()
                .filter_map(|f| {
                    if f.args.len() >= 2 {
                        if let Value::String(input) = &f.args[1] {
                            return Some(input.clone());
                        }
                    }
                    None
                })
                .collect();

            let mut hash_facts = Vec::new();
            for source in source_paths.iter().chain(build_input_paths.iter()) {
                let path = Path::new(source);
                if path.exists() {
                    if source_paths.contains(source) {
                        hash_facts.push(crate::datalog::Fact {
                            predicate: "file_exists".to_string(),
                            args: vec![Value::String(source.clone())],
                        });
                    }

                    if let Ok(contents) = std::fs::read(path) {
                        let hash = xxhash_rust::xxh3::xxh3_64(&contents);
                        hash_facts.push(crate::datalog::Fact {
                            predicate: "file_hash".to_string(),
                            args: vec![
                                Value::String(source.clone()),
                                Value::String(format!("{:016x}", hash)),
                            ],
                        });
                    }
                }
            }
            db.insert_facts(hash_facts);

            let attr_facts = db.query("attr", &[Some(&target_str)]);
            let mut attrs: Vec<(String, String)> = Vec::new();

            for attr_fact in attr_facts {
                if attr_fact.args.len() >= 3 {
                    if let (Value::String(key), Value::String(value)) =
                        (&attr_fact.args[1], &attr_fact.args[2])
                    {
                        attrs.push((key.clone(), value.clone()));
                    }
                }
            }

            attrs.sort();

            let mut attrs_str = String::new();
            for (key, value) in attrs {
                attrs_str.push_str(&key);
                attrs_str.push('=');
                attrs_str.push_str(&value);
                attrs_str.push(';');
            }

            let mut attr_toolchain_facts = Vec::new();
            let attrs_hash = xxhash_rust::xxh3::xxh3_64(attrs_str.as_bytes());
            attr_toolchain_facts.push(crate::datalog::Fact {
                predicate: "file_hash".to_string(),
                args: vec![
                    Value::String("__attrs__".to_string()),
                    Value::String(format!("{:016x}", attrs_hash)),
                ],
            });

            let toolchain_facts = db.query("toolchain", &[]);
            for toolchain_fact in toolchain_facts {
                if toolchain_fact.args.len() >= 3 {
                    if let (Value::String(t), Value::String(tool), Value::String(path)) = (
                        &toolchain_fact.args[0],
                        &toolchain_fact.args[1],
                        &toolchain_fact.args[2],
                    ) {
                        if t == &target_str {
                            let tool_hash = xxhash_rust::xxh3::xxh3_64(path.as_bytes());
                            attr_toolchain_facts.push(crate::datalog::Fact {
                                predicate: "file_hash".to_string(),
                                args: vec![
                                    Value::String(format!("__tool_{}__", tool)),
                                    Value::String(format!("{:016x}", tool_hash)),
                                ],
                            });
                        }
                    }
                }
            }

            db.insert_facts(attr_toolchain_facts);

            let cached_hashes = cache::load_cached_hashes(target);
            db.insert_facts(cached_hashes);

            let needs_rebuild_facts = db.query("needs_rebuild", &[]);
            let needs_rebuild = needs_rebuild_facts.iter().any(|f| {
                if let Some(Value::String(t)) = f.args.first() {
                    t == &target.to_string()
                } else {
                    false
                }
            });

            if !needs_rebuild {
                println!("{} is up to date", target);
                continue;
            }

            let kind_facts = db.query("kind", &[]);
            let mut target_kind = None;

            for fact in kind_facts {
                if fact.args.len() >= 2 {
                    if let Value::String(label_str) = &fact.args[0] {
                        if label_str == &target.to_string() {
                            if let Value::String(kind) = &fact.args[1] {
                                target_kind = Some(kind.clone());
                                break;
                            }
                        }
                    }
                }
            }

            let kind =
                target_kind.ok_or_else(|| format!("No kind defined for target {}", target))?;

            let build_kind = registry
                .get(&kind)
                .ok_or_else(|| format!("Unknown build kind: {}", kind))?;

            let plan = build_kind.plan(target, &mut db)?;
            targets_to_build.push((target.clone(), plan));
        }

        total_planning_time += planning_start.elapsed();

        check_output_conflicts(&targets_to_build)?;

        let execution_start = std::time::Instant::now();
        let build_results: Result<Vec<_>, String> = targets_to_build
            .par_iter()
            .map(|(target, plan)| {
                if dry_run {
                    println!("[DRY RUN] Would build: {}", target);
                    println!("  Command: {}", plan.command);
                    if !plan.args.is_empty() {
                        println!("  Args: {}", plan.args.join(" "));
                    }
                    if !plan.inputs.is_empty() {
                        let inputs: Vec<String> = plan
                            .inputs
                            .iter()
                            .map(|p| p.display().to_string())
                            .collect();
                        println!("  Inputs: {}", inputs.join(", "));
                    }
                    if !plan.outputs.is_empty() {
                        let outputs: Vec<String> = plan
                            .outputs
                            .iter()
                            .map(|p| p.display().to_string())
                            .collect();
                        println!("  Outputs: {}", outputs.join(", "));
                    }
                    Ok(target.clone())
                } else {
                    let result = executor.execute(plan)?;
                    if !result.success {
                        let exit_info = match result.exit_code {
                            Some(code) => format!("exit code {}", code),
                            None => "terminated by signal".to_string(),
                        };
                        return Err(format!(
                            "Target '{}': Build failed with {}\n\
                             Suggestion: Check the error output above for details",
                            target, exit_info
                        ));
                    }
                    Ok(target.clone())
                }
            })
            .collect();

        let built_targets = build_results?;
        total_execution_time += execution_start.elapsed();

        for target in built_targets {
            let source_hashes = collect_source_hashes(&target, &mut db);
            cache::save_build_hashes(&target, &source_hashes)?;
        }
    }

    let total_time = total_start.elapsed();

    eprintln!("\nBuild Profile:");
    eprintln!(
        "  Engine init:       {:>8.3}ms",
        init_time.as_secs_f64() * 1000.0
    );
    eprintln!(
        "  BUILD.datalog:     {:>8.3}ms",
        parse_time.as_secs_f64() * 1000.0
    );
    eprintln!(
        "  Toolchain scan:    {:>8.3}ms",
        toolchain_time.as_secs_f64() * 1000.0
    );
    eprintln!(
        "  Prelude load:      {:>8.3}ms",
        prelude_time.as_secs_f64() * 1000.0
    );
    eprintln!(
        "  --with inject:     {:>8.3}ms",
        with_inject_time.as_secs_f64() * 1000.0
    );
    eprintln!(
        "  File scan:         {:>8.3}ms",
        file_scan_time.as_secs_f64() * 1000.0
    );
    eprintln!(
        "  Topological sort:  {:>8.3}ms",
        topo_time.as_secs_f64() * 1000.0
    );
    eprintln!(
        "  Planning phase:    {:>8.3}ms",
        total_planning_time.as_secs_f64() * 1000.0
    );
    eprintln!(
        "  Execution phase:   {:>8.3}ms",
        total_execution_time.as_secs_f64() * 1000.0
    );
    eprintln!(
        "  Total:             {:>8.3}ms",
        total_time.as_secs_f64() * 1000.0
    );

    Ok(())
}

fn check_output_conflicts(targets: &[(TargetLabel, BuildPlan)]) -> Result<(), String> {
    let mut outputs_map: HashMap<PathBuf, Vec<TargetLabel>> = HashMap::new();

    for (target, plan) in targets {
        for output in &plan.outputs {
            outputs_map
                .entry(output.clone())
                .or_default()
                .push(target.clone());
        }
    }

    let conflicts: Vec<_> = outputs_map
        .iter()
        .filter(|(_, targets)| targets.len() > 1)
        .collect();

    if !conflicts.is_empty() {
        let mut msg = String::from("Output conflicts detected:\n");
        for (path, targets) in conflicts {
            msg.push_str(&format!("  {} declared by:\n", path.display()));
            for target in targets {
                msg.push_str(&format!("    {}\n", target));
            }
        }
        msg.push_str("\nEach output file must be produced by exactly one target.");
        return Err(msg);
    }

    Ok(())
}

fn compute_build_levels(ordered_targets: &[TargetLabel], db: &mut Engine) -> Vec<Vec<TargetLabel>> {
    let deps_facts = db.query("deps", &[]);

    let mut direct_deps: HashMap<String, Vec<String>> = HashMap::new();
    for fact in &deps_facts {
        if fact.args.len() >= 2 {
            if let (Value::String(target), Value::String(dep)) = (&fact.args[0], &fact.args[1]) {
                direct_deps
                    .entry(target.clone())
                    .or_default()
                    .push(dep.clone());
            }
        }
    }

    let target_strings: Vec<String> = ordered_targets.iter().map(|t| t.to_string()).collect();
    let mut level_map: HashMap<String, usize> = HashMap::new();

    for target_str in &target_strings {
        let mut max_dep_level = 0;
        if let Some(deps) = direct_deps.get(target_str) {
            for dep in deps {
                if let Some(&dep_level) = level_map.get(dep) {
                    max_dep_level = max_dep_level.max(dep_level + 1);
                }
            }
        }
        level_map.insert(target_str.clone(), max_dep_level);
    }

    let max_level = level_map.values().max().copied().unwrap_or(0);
    let mut levels: Vec<Vec<TargetLabel>> = vec![Vec::new(); max_level + 1];

    for target in ordered_targets {
        let target_str = target.to_string();
        if let Some(&level) = level_map.get(&target_str) {
            levels[level].push(target.clone());
        }
    }

    levels
}

fn topological_sort(
    targets: &[TargetLabel],
    db: &mut Engine,
    loader: &mut NamespaceLoader,
) -> Result<Vec<TargetLabel>, String> {
    let mut dependencies: HashMap<String, Vec<String>> = HashMap::new();
    let mut to_visit: Vec<String> = targets.iter().map(|t| t.to_string()).collect();
    let mut visited_queries: std::collections::HashSet<String> = std::collections::HashSet::new();

    while let Some(target_str) = to_visit.pop() {
        if visited_queries.contains(&target_str) {
            continue;
        }
        visited_queries.insert(target_str.clone());

        // Lazy load namespace if needed
        let namespace = extract_namespace(&target_str);
        loader.load_namespace(&namespace, db)?;

        // Query direct deps and manually compute transitivity in Rust.
        // Note: We can't use transitive_deps here because the engine's TC lazy
        // evaluation requires base facts for the edge predicate, but deps can be
        // derived (e.g. via alias propagation rules). This approach works for both
        // base and derived deps.
        let deps_for_target = db.query("deps", &[Some(&target_str)]);

        for fact in &deps_for_target {
            if fact.args.len() >= 2 {
                if let (Value::String(target), Value::String(dep)) = (&fact.args[0], &fact.args[1])
                {
                    dependencies
                        .entry(target.clone())
                        .or_default()
                        .push(dep.clone());

                    if !visited_queries.contains(dep) {
                        to_visit.push(dep.clone());
                    }
                }
            }
        }
    }

    let mut result = Vec::new();
    let mut visited: HashMap<String, bool> = HashMap::new();
    let mut visiting: HashMap<String, bool> = HashMap::new();

    fn visit(
        target: &str,
        dependencies: &HashMap<String, Vec<String>>,
        visited: &mut HashMap<String, bool>,
        visiting: &mut HashMap<String, bool>,
        result: &mut Vec<String>,
    ) -> Result<(), String> {
        if *visited.get(target).unwrap_or(&false) {
            return Ok(());
        }

        if *visiting.get(target).unwrap_or(&false) {
            return Err(format!("Circular dependency detected involving {}", target));
        }

        visiting.insert(target.to_string(), true);

        if let Some(deps) = dependencies.get(target) {
            for dep in deps {
                visit(dep, dependencies, visited, visiting, result)?;
            }
        }

        visiting.insert(target.to_string(), false);
        visited.insert(target.to_string(), true);
        result.push(target.to_string());
        Ok(())
    }

    for target in targets {
        let target_str = target.to_string();
        visit(
            &target_str,
            &dependencies,
            &mut visited,
            &mut visiting,
            &mut result,
        )?;
    }

    // Convert all targets (requested + dependencies) to TargetLabel
    // Result is already in correct order (dependencies first)
    result.into_iter().map(|s| TargetLabel::parse(&s)).collect()
}

fn collect_source_hashes(target: &TargetLabel, db: &mut Engine) -> HashMap<String, String> {
    let mut hashes = HashMap::new();

    let sources_facts = db.query("sources", &[]);
    let build_input_facts = db.query("build_input", &[]);
    let hash_facts = db.query("file_hash", &[]);

    for sources_fact in sources_facts.iter().chain(build_input_facts.iter()) {
        if sources_fact.args.len() >= 2 {
            if let (Value::String(t), Value::String(source)) =
                (&sources_fact.args[0], &sources_fact.args[1])
            {
                if t == &target.to_string() {
                    for hash_fact in &hash_facts {
                        if hash_fact.args.len() >= 2 {
                            if let (Value::String(path), Value::String(hash)) =
                                (&hash_fact.args[0], &hash_fact.args[1])
                            {
                                if path == source {
                                    hashes.insert(source.clone(), hash.clone());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let attr_facts = db.query("attr", &[]);
    let mut attrs: Vec<(String, String)> = Vec::new();

    for attr_fact in attr_facts {
        if attr_fact.args.len() >= 3 {
            if let (Value::String(t), Value::String(key), Value::String(value)) =
                (&attr_fact.args[0], &attr_fact.args[1], &attr_fact.args[2])
            {
                if t == &target.to_string() {
                    attrs.push((key.clone(), value.clone()));
                }
            }
        }
    }

    attrs.sort();

    let mut attrs_str = String::new();
    for (key, value) in attrs {
        attrs_str.push_str(&key);
        attrs_str.push('=');
        attrs_str.push_str(&value);
        attrs_str.push(';');
    }

    let attrs_hash = xxhash_rust::xxh3::xxh3_64(attrs_str.as_bytes());
    hashes.insert("__attrs__".to_string(), format!("{:016x}", attrs_hash));

    let toolchain_facts = db.query("toolchain", &[]);
    for toolchain_fact in toolchain_facts {
        if toolchain_fact.args.len() >= 3 {
            if let (Value::String(t), Value::String(tool), Value::String(path)) = (
                &toolchain_fact.args[0],
                &toolchain_fact.args[1],
                &toolchain_fact.args[2],
            ) {
                if t == &target.to_string() {
                    let tool_hash = xxhash_rust::xxh3::xxh3_64(path.as_bytes());
                    hashes.insert(format!("__tool_{}__", tool), format!("{:016x}", tool_hash));
                }
            }
        }
    }

    let deps_facts = db.query("deps", &[]);
    let outputs_facts = db.query("outputs", &[]);
    let target_str = target.to_string();

    for deps_fact in &deps_facts {
        if deps_fact.args.len() < 2 {
            continue;
        }

        let (Some(Value::String(t)), Some(Value::String(dep))) =
            (deps_fact.args.first(), deps_fact.args.get(1))
        else {
            continue;
        };

        if t != &target_str {
            continue;
        }

        for outputs_fact in &outputs_facts {
            if outputs_fact.args.len() < 2 {
                continue;
            }

            let (Some(Value::String(dep_target)), Some(Value::String(output))) =
                (outputs_fact.args.first(), outputs_fact.args.get(1))
            else {
                continue;
            };

            if dep_target != dep {
                continue;
            }

            for hash_fact in &hash_facts {
                if hash_fact.args.len() < 2 {
                    continue;
                }

                let (Some(Value::String(path)), Some(Value::String(hash))) =
                    (hash_fact.args.first(), hash_fact.args.get(1))
                else {
                    continue;
                };

                if path == output {
                    hashes.insert(output.clone(), hash.clone());
                    break;
                }
            }
        }
    }

    hashes
}

fn generate_file_exists_facts() -> Vec<Fact> {
    let mut facts = Vec::new();

    let paths_to_scan = ["src", "tests", "benches", "prelude", "scripts"];

    for path in &paths_to_scan {
        if let Ok(entries) = glob::glob(&format!("{}/**/*", path)) {
            for entry in entries.flatten() {
                if entry.is_file() {
                    if let Some(path_str) = entry.to_str() {
                        facts.push(Fact {
                            predicate: "file_exists".to_string(),
                            args: vec![Value::String(path_str.to_string())],
                        });
                    }
                }
            }
        }
    }

    facts
}
