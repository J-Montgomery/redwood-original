use crate::datalog::{Fact, TargetLabel, Value};
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const CACHE_DIR: &str = ".redwood/cache";
const CACHE_DB: &str = "build_cache.db";

thread_local! {
    static CONN_CACHE: Mutex<Option<Connection>> = const { Mutex::new(None) };
}

/// Find the workspace root by searching upward for the topmost directory containing BUILD.datalog.
/// This ensures the cache is always at the true workspace root, not in a subdirectory package.
/// Returns the current directory if no BUILD.datalog is found.
fn find_workspace_root() -> PathBuf {
    let current = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut dir = current.as_path();
    let mut topmost_root: Option<PathBuf> = None;

    loop {
        if dir.join("BUILD.datalog").exists() {
            topmost_root = Some(dir.to_path_buf());
        }

        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }

    // Return topmost BUILD.datalog location, or current directory if none found
    topmost_root.unwrap_or(current)
}

fn get_cache_db_path(base_dir: &Path) -> PathBuf {
    base_dir.join(CACHE_DIR).join(CACHE_DB)
}

fn get_connection(base_dir: &Path) -> Result<Connection, String> {
    let db_path = get_cache_db_path(base_dir);

    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Cache initialization failed\n\
                 Directory: {}\n\
                 Error: {}",
                parent.display(),
                e
            )
        })?;
    }

    let conn = Connection::open(&db_path).map_err(|e| {
        format!(
            "Cache database error\n\
             Database path: {}\n\
             Error: {}",
            db_path.display(),
            e
        )
    })?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS cache (
            target TEXT NOT NULL,
            file TEXT NOT NULL,
            hash TEXT NOT NULL,
            PRIMARY KEY (target, file)
        )",
        [],
    )
    .map_err(|e| {
        format!(
            "Cache database initialization failed\n\
             Database path: {}\n\
             Error: {}",
            db_path.display(),
            e
        )
    })?;

    Ok(conn)
}

pub fn load_cached_hashes(target: &TargetLabel) -> Vec<Fact> {
    load_cached_hashes_from_dir(&find_workspace_root(), target)
}

fn load_cached_hashes_from_dir(base_dir: &Path, target: &TargetLabel) -> Vec<Fact> {
    let conn = match get_connection(base_dir) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let target_str = target.to_string();
    let mut stmt = match conn.prepare("SELECT file, hash FROM cache WHERE target = ?1") {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    let rows = match stmt.query_map([&target_str], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) {
        Ok(r) => r,
        Err(_) => return vec![],
    };

    let mut facts = Vec::new();
    for (file, hash) in rows.flatten() {
        facts.push(Fact {
            predicate: "cached_hash".to_string(),
            args: vec![
                Value::String(target_str.clone()),
                Value::String(file),
                Value::String(hash),
            ],
        });
    }

    facts
}

pub fn save_build_hashes(
    target: &TargetLabel,
    source_hashes: &HashMap<String, String>,
) -> Result<(), String> {
    save_build_hashes_in_dir(&find_workspace_root(), target, source_hashes)
}

fn save_build_hashes_in_dir(
    base_dir: &Path,
    target: &TargetLabel,
    source_hashes: &HashMap<String, String>,
) -> Result<(), String> {
    let conn = get_connection(base_dir)?;
    let target_str = target.to_string();

    let tx = conn.unchecked_transaction().map_err(|e| {
        format!(
            "Target '{}': Cache transaction failed\n\
             Database: {}\n\
             Error: {}",
            target_str,
            get_cache_db_path(base_dir).display(),
            e
        )
    })?;

    tx.execute("DELETE FROM cache WHERE target = ?1", params![&target_str])
        .map_err(|e| {
            format!(
                "Target '{}': Failed to clear cache entries\n\
                 Database: {}\n\
                 Error: {}",
                target_str,
                get_cache_db_path(base_dir).display(),
                e
            )
        })?;

    for (file, hash) in source_hashes {
        tx.execute(
            "INSERT INTO cache (target, file, hash) VALUES (?1, ?2, ?3)",
            params![&target_str, file, hash],
        )
        .map_err(|e| {
            format!(
                "Target '{}': Failed to save cache entry\n\
                 File: {}\n\
                 Hash: {}\n\
                 Database: {}\n\
                 Error: {}",
                target_str,
                file,
                hash,
                get_cache_db_path(base_dir).display(),
                e
            )
        })?;
    }

    tx.commit().map_err(|e| {
        format!(
            "Target '{}': Failed to commit cache transaction\n\
             Database: {}\n\
             Entries: {}\n\
             Error: {}",
            target_str,
            get_cache_db_path(base_dir).display(),
            source_hashes.len(),
            e
        )
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn save_and_load_hashes() {
        let dir = tempdir().unwrap();

        let target = TargetLabel::parse("//app:cli").unwrap();
        let mut hashes = HashMap::new();
        hashes.insert("src/main.rs".to_string(), "abc123".to_string());
        hashes.insert("src/lib.rs".to_string(), "def456".to_string());

        save_build_hashes_in_dir(dir.path(), &target, &hashes).unwrap();

        let cached_facts = load_cached_hashes_from_dir(dir.path(), &target);
        assert_eq!(cached_facts.len(), 2);

        for fact in &cached_facts {
            assert_eq!(fact.predicate, "cached_hash");
            assert_eq!(fact.args[0], Value::String("//app:cli".to_string()));
        }
    }

    #[test]
    fn load_nonexistent_cache() {
        let dir = tempdir().unwrap();

        let target = TargetLabel::parse("//app:nonexistent").unwrap();
        let cached_facts = load_cached_hashes_from_dir(dir.path(), &target);
        assert_eq!(cached_facts.len(), 0);
    }

    #[test]
    fn cache_persistence_across_cycles() {
        let dir = tempdir().unwrap();
        let target = TargetLabel::parse("//app:binary").unwrap();

        let mut hashes = HashMap::new();
        hashes.insert("src/main.rs".to_string(), "hash1".to_string());
        hashes.insert("src/lib.rs".to_string(), "hash2".to_string());

        save_build_hashes_in_dir(dir.path(), &target, &hashes).unwrap();

        let cached = load_cached_hashes_from_dir(dir.path(), &target);
        assert_eq!(cached.len(), 2);

        let cached_again = load_cached_hashes_from_dir(dir.path(), &target);
        assert_eq!(cached_again.len(), 2);
        assert_eq!(cached, cached_again);
    }

    #[test]
    fn cache_overwrite_with_different_hashes() {
        let dir = tempdir().unwrap();
        let target = TargetLabel::parse("//app:cli").unwrap();

        let mut hashes1 = HashMap::new();
        hashes1.insert("src/main.rs".to_string(), "old_hash".to_string());

        save_build_hashes_in_dir(dir.path(), &target, &hashes1).unwrap();

        let cached1 = load_cached_hashes_from_dir(dir.path(), &target);
        assert_eq!(cached1.len(), 1);
        assert_eq!(cached1[0].args[2], Value::String("old_hash".to_string()));

        let mut hashes2 = HashMap::new();
        hashes2.insert("src/main.rs".to_string(), "new_hash".to_string());

        save_build_hashes_in_dir(dir.path(), &target, &hashes2).unwrap();

        let cached2 = load_cached_hashes_from_dir(dir.path(), &target);
        assert_eq!(cached2.len(), 1);
        assert_eq!(cached2[0].args[2], Value::String("new_hash".to_string()));
    }

    #[test]
    fn cache_multiple_targets_independently() {
        let dir = tempdir().unwrap();

        let target1 = TargetLabel::parse("//app:binary1").unwrap();
        let target2 = TargetLabel::parse("//app:binary2").unwrap();

        let mut hashes1 = HashMap::new();
        hashes1.insert("src/main1.rs".to_string(), "hash1".to_string());

        let mut hashes2 = HashMap::new();
        hashes2.insert("src/main2.rs".to_string(), "hash2".to_string());

        save_build_hashes_in_dir(dir.path(), &target1, &hashes1).unwrap();
        save_build_hashes_in_dir(dir.path(), &target2, &hashes2).unwrap();

        let cached1 = load_cached_hashes_from_dir(dir.path(), &target1);
        let cached2 = load_cached_hashes_from_dir(dir.path(), &target2);

        assert_eq!(cached1.len(), 1);
        assert_eq!(cached2.len(), 1);

        assert_eq!(
            cached1[0].args[1],
            Value::String("src/main1.rs".to_string())
        );
        assert_eq!(
            cached2[0].args[1],
            Value::String("src/main2.rs".to_string())
        );
    }

    #[test]
    fn cache_file_addition_and_removal() {
        let dir = tempdir().unwrap();
        let target = TargetLabel::parse("//app:test").unwrap();

        let mut hashes1 = HashMap::new();
        hashes1.insert("src/a.rs".to_string(), "hash_a".to_string());
        hashes1.insert("src/b.rs".to_string(), "hash_b".to_string());

        save_build_hashes_in_dir(dir.path(), &target, &hashes1).unwrap();

        let cached1 = load_cached_hashes_from_dir(dir.path(), &target);
        assert_eq!(cached1.len(), 2);

        let mut hashes2 = HashMap::new();
        hashes2.insert("src/a.rs".to_string(), "hash_a".to_string());
        hashes2.insert("src/b.rs".to_string(), "hash_b".to_string());
        hashes2.insert("src/c.rs".to_string(), "hash_c".to_string());

        save_build_hashes_in_dir(dir.path(), &target, &hashes2).unwrap();

        let cached2 = load_cached_hashes_from_dir(dir.path(), &target);
        assert_eq!(cached2.len(), 3);

        let mut hashes3 = HashMap::new();
        hashes3.insert("src/a.rs".to_string(), "hash_a".to_string());

        save_build_hashes_in_dir(dir.path(), &target, &hashes3).unwrap();

        let cached3 = load_cached_hashes_from_dir(dir.path(), &target);
        assert_eq!(cached3.len(), 1);
        assert_eq!(cached3[0].args[1], Value::String("src/a.rs".to_string()));
    }

    #[test]
    fn cache_handles_empty_hash_set() {
        let dir = tempdir().unwrap();
        let target = TargetLabel::parse("//app:empty").unwrap();

        let hashes = HashMap::new();

        save_build_hashes_in_dir(dir.path(), &target, &hashes).unwrap();

        let cached = load_cached_hashes_from_dir(dir.path(), &target);
        assert_eq!(cached.len(), 0);
    }

    #[test]
    fn cache_target_label_variations() {
        let dir = tempdir().unwrap();

        let target1 = TargetLabel::parse("//app:cli").unwrap();
        let target2 = TargetLabel::parse("//app/subdir:cli").unwrap();
        let target3 = TargetLabel::parse("//other:cli").unwrap();

        let mut hashes = HashMap::new();
        hashes.insert("test.rs".to_string(), "unique_hash".to_string());

        save_build_hashes_in_dir(dir.path(), &target1, &hashes).unwrap();
        save_build_hashes_in_dir(dir.path(), &target2, &hashes).unwrap();
        save_build_hashes_in_dir(dir.path(), &target3, &hashes).unwrap();

        let cached1 = load_cached_hashes_from_dir(dir.path(), &target1);
        let cached2 = load_cached_hashes_from_dir(dir.path(), &target2);
        let cached3 = load_cached_hashes_from_dir(dir.path(), &target3);

        assert_eq!(cached1.len(), 1);
        assert_eq!(cached2.len(), 1);
        assert_eq!(cached3.len(), 1);

        assert_eq!(cached1[0].args[0], Value::String("//app:cli".to_string()));
        assert_eq!(
            cached2[0].args[0],
            Value::String("//app/subdir:cli".to_string())
        );
        assert_eq!(cached3[0].args[0], Value::String("//other:cli".to_string()));
    }

    #[test]
    fn cache_directory_created_automatically() {
        let dir = tempdir().unwrap();
        let target = TargetLabel::parse("//deep/nested/path:binary").unwrap();

        let mut hashes = HashMap::new();
        hashes.insert("src/main.rs".to_string(), "hash".to_string());

        let result = save_build_hashes_in_dir(dir.path(), &target, &hashes);
        assert!(result.is_ok());

        let db_path = get_cache_db_path(dir.path());
        assert!(db_path.exists());

        let cached = load_cached_hashes_from_dir(dir.path(), &target);
        assert_eq!(cached.len(), 1);
    }

    #[test]
    fn cache_fact_format_correctness() {
        let dir = tempdir().unwrap();
        let target = TargetLabel::parse("//project:lib").unwrap();

        let mut hashes = HashMap::new();
        hashes.insert("src/file.rs".to_string(), "abc123def456".to_string());

        save_build_hashes_in_dir(dir.path(), &target, &hashes).unwrap();

        let cached = load_cached_hashes_from_dir(dir.path(), &target);
        assert_eq!(cached.len(), 1);

        let fact = &cached[0];
        assert_eq!(fact.predicate, "cached_hash");
        assert_eq!(fact.args.len(), 3);
        assert_eq!(fact.args[0], Value::String("//project:lib".to_string()));
        assert_eq!(fact.args[1], Value::String("src/file.rs".to_string()));
        assert_eq!(fact.args[2], Value::String("abc123def456".to_string()));
    }

    #[test]
    fn concurrent_cache_writes() {
        use std::thread;

        let dir = tempdir().unwrap();
        let dir_path = dir.path().to_path_buf();

        let handles: Vec<_> = (0..10)
            .map(|i| {
                let path = dir_path.clone();
                thread::spawn(move || {
                    let target = TargetLabel::parse(&format!("//app:target{}", i)).unwrap();
                    let mut hashes = HashMap::new();
                    hashes.insert(format!("file{}.rs", i), format!("hash_{}", i));
                    save_build_hashes_in_dir(&path, &target, &hashes).unwrap();
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        for i in 0..10 {
            let target = TargetLabel::parse(&format!("//app:target{}", i)).unwrap();
            let cached = load_cached_hashes_from_dir(&dir_path, &target);
            assert_eq!(cached.len(), 1, "Target {} should have 1 cache entry", i);
            assert_eq!(cached[0].args[2], Value::String(format!("hash_{}", i)));
        }
    }

    #[test]
    fn concurrent_read_write() {
        use std::sync::Arc;
        use std::thread;

        let dir = tempdir().unwrap();
        let dir_path = Arc::new(dir.path().to_path_buf());

        let target = TargetLabel::parse("//app:concurrent").unwrap();
        let mut initial_hashes = HashMap::new();
        initial_hashes.insert("initial.rs".to_string(), "initial_hash".to_string());
        save_build_hashes_in_dir(&dir_path, &target, &initial_hashes).unwrap();

        let mut handles = vec![];

        for i in 0..5 {
            let path = Arc::clone(&dir_path);
            let target_clone = target.clone();
            handles.push(thread::spawn(move || {
                let cached = load_cached_hashes_from_dir(&path, &target_clone);
                assert!(!cached.is_empty(), "Reader {} should see cache", i);
            }));
        }

        for i in 0..5 {
            let path = Arc::clone(&dir_path);
            let target_clone = target.clone();
            handles.push(thread::spawn(move || {
                let mut hashes = HashMap::new();
                hashes.insert(format!("file{}.rs", i), format!("hash_{}", i));
                save_build_hashes_in_dir(&path, &target_clone, &hashes).unwrap();
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let final_cached = load_cached_hashes_from_dir(&dir_path, &target);
        assert_eq!(
            final_cached.len(),
            1,
            "Final cache should have 1 entry from last write"
        );
    }

    #[test]
    fn cache_write_atomicity() {
        let dir = tempdir().unwrap();
        let target = TargetLabel::parse("//app:atomic").unwrap();

        let mut hashes1 = HashMap::new();
        hashes1.insert("a.rs".to_string(), "hash_a".to_string());
        hashes1.insert("b.rs".to_string(), "hash_b".to_string());
        hashes1.insert("c.rs".to_string(), "hash_c".to_string());

        save_build_hashes_in_dir(dir.path(), &target, &hashes1).unwrap();
        let cached1 = load_cached_hashes_from_dir(dir.path(), &target);
        assert_eq!(cached1.len(), 3);

        let mut hashes2 = HashMap::new();
        hashes2.insert("x.rs".to_string(), "hash_x".to_string());

        save_build_hashes_in_dir(dir.path(), &target, &hashes2).unwrap();
        let cached2 = load_cached_hashes_from_dir(dir.path(), &target);
        assert_eq!(cached2.len(), 1);
        assert_eq!(cached2[0].args[1], Value::String("x.rs".to_string()));

        let has_old_files = cached2.iter().any(|f| {
            if let Value::String(file) = &f.args[1] {
                file == "a.rs" || file == "b.rs" || file == "c.rs"
            } else {
                false
            }
        });
        assert!(
            !has_old_files,
            "Old cache entries should be completely replaced"
        );
    }
}
