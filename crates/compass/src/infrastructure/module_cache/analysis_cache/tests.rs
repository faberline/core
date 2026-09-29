use super::*;
use crate::domain::module_cache::content_hash::ContentHash;
use crate::domain::modules::import::ModuleInfo;
use std::env;
use std::fs;
use std::path::PathBuf;

#[test]
fn test_content_hash() {
    let hash1 = ContentHash::from_content("hello world");
    let hash2 = ContentHash::from_content("hello world");
    let hash3 = ContentHash::from_content("hello world!");

    assert_eq!(hash1, hash2);
    assert_ne!(hash1, hash3);
}

#[test]
fn test_cache_entry_creation() {
    let path = PathBuf::from("/test/module.py");
    let hash = ContentHash::from_content("content");
    let info = ModuleInfo::new("test_module");

    let entry = CacheEntry::new("test_module".to_string(), path.clone(), hash, info);

    assert_eq!(entry.module_name, "test_module");
    assert_eq!(entry.path, path);
    assert_eq!(entry.content_hash, hash);
}

#[test]
fn test_analysis_cache_store_and_get() {
    let mut cache = AnalysisCache::new();

    let entry = CacheEntry::new(
        "mymodule".to_string(),
        PathBuf::from("/test/mymodule.py"),
        ContentHash::from_content("content"),
        ModuleInfo::new("mymodule"),
    );

    cache.store(entry);

    assert!(cache.has("mymodule"));
    assert!(!cache.has("nonexistent"));

    let retrieved = cache.get("mymodule");
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().module_name, "mymodule");
}

#[test]
fn test_reverse_dependencies() {
    let mut cache = AnalysisCache::new();

    // Create module B that depends on A
    let mut entry_b = CacheEntry::new(
        "b".to_string(),
        PathBuf::from("/test/b.py"),
        ContentHash::from_content("import a"),
        ModuleInfo::new("b"),
    );
    entry_b.dependencies.insert("a".to_string());

    // Create module C that depends on A
    let mut entry_c = CacheEntry::new(
        "c".to_string(),
        PathBuf::from("/test/c.py"),
        ContentHash::from_content("import a"),
        ModuleInfo::new("c"),
    );
    entry_c.dependencies.insert("a".to_string());

    cache.store(entry_b);
    cache.store(entry_c);

    // Check affected modules when A changes
    let affected = cache.get_affected_modules("a");
    assert!(affected.contains("a"));
    assert!(affected.contains("b"));
    assert!(affected.contains("c"));
}

#[test]
fn test_transitive_dependencies() {
    let mut cache = AnalysisCache::new();

    // A -> B -> C (chain of dependencies)
    let entry_a = CacheEntry::new(
        "a".to_string(),
        PathBuf::from("/test/a.py"),
        ContentHash::from_content(""),
        ModuleInfo::new("a"),
    );

    let mut entry_b = CacheEntry::new(
        "b".to_string(),
        PathBuf::from("/test/b.py"),
        ContentHash::from_content("import a"),
        ModuleInfo::new("b"),
    );
    entry_b.dependencies.insert("a".to_string());

    let mut entry_c = CacheEntry::new(
        "c".to_string(),
        PathBuf::from("/test/c.py"),
        ContentHash::from_content("import b"),
        ModuleInfo::new("c"),
    );
    entry_c.dependencies.insert("b".to_string());

    cache.store(entry_a);
    cache.store(entry_b);
    cache.store(entry_c);

    // When A changes, both B and C should be affected
    let affected = cache.get_affected_modules("a");
    assert!(affected.contains("a"));
    assert!(affected.contains("b"));
    assert!(affected.contains("c"));
}

#[test]
fn test_invalidation() {
    let mut cache = AnalysisCache::new();

    let entry_a = CacheEntry::new(
        "a".to_string(),
        PathBuf::from("/test/a.py"),
        ContentHash::from_content(""),
        ModuleInfo::new("a"),
    );

    let mut entry_b = CacheEntry::new(
        "b".to_string(),
        PathBuf::from("/test/b.py"),
        ContentHash::from_content("import a"),
        ModuleInfo::new("b"),
    );
    entry_b.dependencies.insert("a".to_string());

    cache.store(entry_a);
    cache.store(entry_b);

    assert!(cache.has("a"));
    assert!(cache.has("b"));

    // Invalidate A, which should also invalidate B
    cache.invalidate("a");

    assert!(!cache.has("a"));
    assert!(!cache.has("b"));
}

#[test]
fn test_cache_stats() {
    let mut cache = AnalysisCache::new();

    let entry = CacheEntry::new(
        "module".to_string(),
        PathBuf::from("/test/module.py"),
        ContentHash::from_content(""),
        ModuleInfo::new("module"),
    );

    cache.store(entry);

    let stats = cache.stats();
    assert_eq!(stats.total_entries, 1);
}

#[test]
fn test_file_hash() {
    let temp_dir = env::temp_dir().join("cclab_lens_cache_test");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).unwrap();

    let file_path = temp_dir.join("test.py");
    fs::write(&file_path, "print('hello')").unwrap();

    let hash1 = ContentHash::from_file(&file_path);
    assert!(hash1.is_some());

    // Same content = same hash
    let hash2 = ContentHash::from_file(&file_path);
    assert_eq!(hash1, hash2);

    // Change content = different hash
    fs::write(&file_path, "print('world')").unwrap();
    let hash3 = ContentHash::from_file(&file_path);
    assert_ne!(hash1, hash3);

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

/// R3: After propagation, cache entries can be marked with propagation_valid=true.
#[test]
fn test_cache_stores_propagated_types() {
    let mut cache = AnalysisCache::new();

    // Create a source module.
    let entry_src = CacheEntry::new(
        "db".to_string(),
        PathBuf::from("/test/db.py"),
        ContentHash::from_content("def get_user(): ..."),
        ModuleInfo::new("db"),
    );
    cache.store(entry_src);

    // Create an importing module that depends on db.
    let mut entry_handler = CacheEntry::new(
        "handler".to_string(),
        PathBuf::from("/test/handler.py"),
        ContentHash::from_content("from db import get_user"),
        ModuleInfo::new("handler"),
    );
    entry_handler.dependencies.insert("db".to_string());
    cache.store(entry_handler);

    // Initially propagation_valid should be false.
    assert!(!cache.get("handler").unwrap().propagation_valid);

    // After propagation, mark as valid.
    if let Some(entry) = cache.get_mut("handler") {
        entry.propagation_valid = true;
    }
    assert!(cache.get("handler").unwrap().propagation_valid);
}

/// R8, S6: Changing source file clears propagated bindings in all importers.
#[test]
fn test_invalidation_on_dependency_change() {
    let mut cache = AnalysisCache::new();

    // Module A (source).
    let entry_a = CacheEntry::new(
        "a".to_string(),
        PathBuf::from("/test/a.py"),
        ContentHash::from_content("class Foo: pass"),
        ModuleInfo::new("a"),
    );
    cache.store(entry_a);

    // Module B depends on A.
    let mut entry_b = CacheEntry::new(
        "b".to_string(),
        PathBuf::from("/test/b.py"),
        ContentHash::from_content("from a import Foo"),
        ModuleInfo::new("b"),
    );
    entry_b.dependencies.insert("a".to_string());
    entry_b.propagation_valid = true; // mark as propagated
    cache.store(entry_b);

    // Module C depends on B (transitive).
    let mut entry_c = CacheEntry::new(
        "c".to_string(),
        PathBuf::from("/test/c.py"),
        ContentHash::from_content("from b import Foo"),
        ModuleInfo::new("c"),
    );
    entry_c.dependencies.insert("b".to_string());
    entry_c.propagation_valid = true;
    cache.store(entry_c);

    // Verify both are propagation_valid before invalidation.
    assert!(cache.get("b").unwrap().propagation_valid);
    assert!(cache.get("c").unwrap().propagation_valid);

    // Invalidate propagation starting from A.
    cache.invalidate_propagation("a");

    // Both B and C should have propagation_valid=false now.
    assert!(
        !cache
            .get("a")
            .unwrap_or(&CacheEntry::new(
                "a".to_string(),
                PathBuf::from("/test/a.py"),
                ContentHash::from_content(""),
                ModuleInfo::new("a"),
            ))
            .propagation_valid
    );
    assert!(
        !cache.get("b").unwrap().propagation_valid,
        "B should be invalidated because it depends on A"
    );
    assert!(
        !cache.get("c").unwrap().propagation_valid,
        "C should be invalidated transitively through B"
    );
}
