use std::collections::{HashMap, HashSet};

use crate::infrastructure::module_cache::entry::CacheEntry;

/// Module analysis cache
#[derive(Debug, Default)]
pub struct AnalysisCache {
    /// Cached entries by module name
    entries: HashMap<String, CacheEntry>,
    /// Reverse dependency map: module -> modules that depend on it
    reverse_deps: HashMap<String, HashSet<String>>,
}

impl AnalysisCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            reverse_deps: HashMap::new(),
        }
    }

    /// Store a cache entry
    pub fn store(&mut self, entry: CacheEntry) {
        let module_name = entry.module_name.clone();

        // Update reverse dependency map
        for dep in &entry.dependencies {
            self.reverse_deps
                .entry(dep.clone())
                .or_default()
                .insert(module_name.clone());
        }

        self.entries.insert(module_name, entry);
    }

    /// Get a cached entry
    pub fn get(&self, module_name: &str) -> Option<&CacheEntry> {
        self.entries.get(module_name)
    }

    /// Get mutable cached entry
    pub fn get_mut(&mut self, module_name: &str) -> Option<&mut CacheEntry> {
        self.entries.get_mut(module_name)
    }

    /// Check if module is cached
    pub fn has(&self, module_name: &str) -> bool {
        self.entries.contains_key(module_name)
    }

    /// Remove a cached entry
    pub fn remove(&mut self, module_name: &str) -> Option<CacheEntry> {
        if let Some(entry) = self.entries.remove(module_name) {
            // Clean up reverse deps
            for dep in &entry.dependencies {
                if let Some(rdeps) = self.reverse_deps.get_mut(dep) {
                    rdeps.remove(module_name);
                }
            }
            Some(entry)
        } else {
            None
        }
    }

    /// Get modules that need reanalysis due to changes
    pub fn get_changed_modules(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter(|(_, entry)| entry.needs_reanalysis())
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Get all modules affected by changes to the given module
    /// (including transitive dependents)
    pub fn get_affected_modules(&self, changed: &str) -> HashSet<String> {
        let mut affected = HashSet::new();
        let mut queue = vec![changed.to_string()];

        while let Some(module) = queue.pop() {
            if affected.insert(module.clone()) {
                if let Some(dependents) = self.reverse_deps.get(&module) {
                    for dep in dependents {
                        if !affected.contains(dep) {
                            queue.push(dep.clone());
                        }
                    }
                }
            }
        }

        affected
    }

    /// Invalidate a module and all its dependents
    pub fn invalidate(&mut self, module_name: &str) {
        let affected = self.get_affected_modules(module_name);
        for name in affected {
            self.entries.remove(&name);
        }
    }

    /// Invalidate propagated type information for a module and its dependents (R8).
    ///
    /// Unlike `invalidate()` which removes the entire cache entry, this only
    /// marks the entry for re-propagation while keeping locally-inferred data
    /// intact.  All transitive dependents of `module_name` are also marked.
    pub fn invalidate_propagation(&mut self, module_name: &str) {
        let affected = self.get_affected_modules(module_name);
        for name in affected {
            if let Some(entry) = self.entries.get_mut(&name) {
                entry.propagation_valid = false;
            }
        }
    }

    /// Get all cached module names
    pub fn module_names(&self) -> impl Iterator<Item = &String> {
        self.entries.keys()
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            total_entries: self.entries.len(),
            stale_entries: self.get_changed_modules().len(),
        }
    }

    /// Clear the entire cache
    pub fn clear(&mut self) {
        self.entries.clear();
        self.reverse_deps.clear();
    }
}

/// Cache statistics
#[derive(Debug, Clone, Copy)]
pub struct CacheStats {
    pub total_entries: usize,
    pub stale_entries: usize,
}

#[cfg(test)]
mod tests;
