use crate::datalog::{Fact, Value};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;
use xxhash_rust::xxh3::xxh3_64;

pub fn scan_filesystem(root: &Path) -> Result<Vec<Fact>, String> {
    let mut facts = Vec::new();

    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let path = entry.path();
            let path_str = path
                .strip_prefix(root)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();

            facts.push(Fact {
                predicate: "file_exists".to_string(),
                args: vec![Value::String(path_str.clone())],
            });

            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
                        facts.push(Fact {
                            predicate: "file_modified".to_string(),
                            args: vec![
                                Value::String(path_str.clone()),
                                Value::Integer(duration.as_secs() as i64),
                            ],
                        });
                    }
                }
            }

            if let Ok(contents) = fs::read(path) {
                let hash = xxh3_64(&contents);
                facts.push(Fact {
                    predicate: "file_hash".to_string(),
                    args: vec![
                        Value::String(path_str),
                        Value::String(format!("{:016x}", hash)),
                    ],
                });
            }
        }
    }

    Ok(facts)
}

pub fn generate_glob_facts(file_facts: &[Fact], pattern: &str) -> Vec<Fact> {
    let mut glob_facts = Vec::new();

    for fact in file_facts {
        if fact.predicate == "file_exists" {
            if let Some(Value::String(path)) = fact.args.first() {
                if glob::Pattern::new(pattern)
                    .map(|p| p.matches(path))
                    .unwrap_or(false)
                {
                    glob_facts.push(Fact {
                        predicate: "glob".to_string(),
                        args: vec![
                            Value::String(pattern.to_string()),
                            Value::String(path.clone()),
                        ],
                    });
                }
            }
        }
    }

    glob_facts
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn scan_generates_file_exists_facts() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "content").unwrap();

        let facts = scan_filesystem(dir.path()).unwrap();

        let file_exists_facts: Vec<_> = facts
            .iter()
            .filter(|f| f.predicate == "file_exists")
            .collect();

        assert_eq!(file_exists_facts.len(), 1);
    }

    #[test]
    fn scan_generates_file_modified_facts() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "content").unwrap();

        let facts = scan_filesystem(dir.path()).unwrap();

        let modified_facts: Vec<_> = facts
            .iter()
            .filter(|f| f.predicate == "file_modified")
            .collect();

        assert_eq!(modified_facts.len(), 1);
        match &modified_facts[0].args[1] {
            Value::Integer(ts) => assert!(*ts > 0),
            _ => panic!("Expected Int timestamp"),
        }
    }

    #[test]
    fn glob_predicate_matches_pattern() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("test.rs"), "").unwrap();
        fs::write(dir.path().join("test.txt"), "").unwrap();
        fs::write(dir.path().join("other.rs"), "").unwrap();

        let facts = scan_filesystem(dir.path()).unwrap();
        let glob_facts = generate_glob_facts(&facts, "**/*.rs");

        assert_eq!(glob_facts.len(), 2);
    }

    #[test]
    fn scan_generates_file_hash_facts() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "test content").unwrap();

        let facts = scan_filesystem(dir.path()).unwrap();

        let hash_facts: Vec<_> = facts
            .iter()
            .filter(|f| f.predicate == "file_hash")
            .collect();

        assert_eq!(hash_facts.len(), 1);
        match &hash_facts[0].args[1] {
            Value::String(hash) => {
                assert_eq!(hash.len(), 16);
            }
            _ => panic!("Expected String hash"),
        }
    }

    #[test]
    fn file_hash_changes_with_content() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");

        fs::write(&file_path, "content1").unwrap();
        let facts1 = scan_filesystem(dir.path()).unwrap();
        let hash1 = facts1
            .iter()
            .find(|f| f.predicate == "file_hash")
            .and_then(|f| f.args.get(1))
            .cloned();

        fs::write(&file_path, "content2").unwrap();
        let facts2 = scan_filesystem(dir.path()).unwrap();
        let hash2 = facts2
            .iter()
            .find(|f| f.predicate == "file_hash")
            .and_then(|f| f.args.get(1))
            .cloned();

        assert!(hash1.is_some());
        assert!(hash2.is_some());
        assert_ne!(hash1, hash2);
    }
}
