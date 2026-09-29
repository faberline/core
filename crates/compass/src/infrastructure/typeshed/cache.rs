use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, RwLock};
use std::time::{Duration, SystemTime};

/// Configuration for typeshed integration
#[derive(Debug, Clone)]
pub struct TypeshedConfig {
    /// Custom path to a local typeshed copy (takes precedence over downloads)
    pub typeshed_path: Option<PathBuf>,
    /// Directory to store downloaded stubs
    pub cache_dir: PathBuf,
    /// Disable network requests (offline mode)
    pub offline: bool,
    /// Python version for stub resolution (e.g., "3.10", "3.11")
    pub python_version: String,
    /// Cache TTL in days (default: 7)
    pub cache_ttl_days: u32,
    /// Optional commit hash to pin typeshed version
    pub typeshed_commit: Option<String>,
}

impl Default for TypeshedConfig {
    fn default() -> Self {
        let cache_dir = dirs_cache_dir().unwrap_or_else(|| PathBuf::from(".cclab_lens-cache"));
        Self {
            typeshed_path: None,
            cache_dir,
            offline: false,
            python_version: "3.11".to_string(),
            cache_ttl_days: 7,
            typeshed_commit: None,
        }
    }
}

/// Get platform-specific cache directory
fn dirs_cache_dir() -> Option<PathBuf> {
    // Simple cross-platform cache dir detection
    if let Ok(home) = std::env::var("HOME") {
        Some(PathBuf::from(home).join(".cache").join("cclab_lens"))
    } else if let Ok(cache) = std::env::var("XDG_CACHE_HOME") {
        Some(PathBuf::from(cache).join("cclab_lens"))
    } else {
        None
    }
}

/// Cache entry metadata
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct CacheEntry {
    /// When this entry was last updated
    last_updated: SystemTime,
    /// ETag for conditional requests
    etag: Option<String>,
    /// Python version this stub is for
    python_version: String,
}

/// Typeshed cache manager
///
/// Handles local storage and HTTP fetching of typeshed stubs.
/// Downloads run in background threads to avoid blocking LSP/analysis.
#[derive(Debug)]
pub struct TypeshedCache {
    config: TypeshedConfig,
    /// Cache metadata (module -> entry)
    metadata: RwLock<HashMap<String, CacheEntry>>,
    /// Pending downloads (module names being fetched)
    pending: Mutex<std::collections::HashSet<String>>,
    /// Completed downloads ready to be loaded
    completed: Mutex<Vec<String>>,
}

impl TypeshedCache {
    /// Create a new typeshed cache with the given configuration
    pub fn new(config: TypeshedConfig) -> Self {
        // Ensure cache directory exists
        if !config.cache_dir.exists() {
            let _ = fs::create_dir_all(&config.cache_dir);
        }

        Self {
            config,
            metadata: RwLock::new(HashMap::new()),
            pending: Mutex::new(std::collections::HashSet::new()),
            completed: Mutex::new(Vec::new()),
        }
    }

    /// Get the cache key for a module (includes Python version)
    fn cache_key(&self, module: &str) -> String {
        format!(
            "{}_{}",
            self.config.python_version,
            module.replace('.', "_")
        )
    }

    /// Get the cache file path for a module
    fn cache_path(&self, module: &str) -> PathBuf {
        let key = self.cache_key(module);
        self.config.cache_dir.join(format!("{}.pyi", key))
    }

    /// Check if a cached stub exists and is not expired
    pub fn has_valid_cache(&self, module: &str) -> bool {
        let path = self.cache_path(module);
        if !path.exists() {
            return false;
        }

        // Check TTL
        if let Ok(metadata) = fs::metadata(&path) {
            if let Ok(modified) = metadata.modified() {
                let ttl = Duration::from_secs(self.config.cache_ttl_days as u64 * 24 * 60 * 60);
                if let Ok(elapsed) = SystemTime::now().duration_since(modified) {
                    return elapsed < ttl;
                }
            }
        }

        false
    }

    /// Read cached stub content
    pub fn read_cache(&self, module: &str) -> Option<String> {
        let path = self.cache_path(module);
        fs::read_to_string(path).ok()
    }

    /// Write stub content to cache
    pub fn write_cache(&self, module: &str, content: &str) -> std::io::Result<()> {
        let path = self.cache_path(module);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&path, content)?;

        // Update metadata
        if let Ok(mut metadata) = self.metadata.write() {
            metadata.insert(
                module.to_string(),
                CacheEntry {
                    last_updated: SystemTime::now(),
                    etag: None,
                    python_version: self.config.python_version.clone(),
                },
            );
        }

        Ok(())
    }

    /// Check if a download is already pending for a module
    pub fn is_pending(&self, module: &str) -> bool {
        self.pending
            .lock()
            .map(|p| p.contains(module))
            .unwrap_or(false)
    }

    /// Start a background download for a module (synchronous version for simplicity)
    /// In a real implementation, this would use async or channels for true non-blocking behavior
    /// For now, we download synchronously but the caller can spawn this on a thread pool
    pub fn download_module(&self, module: &str) -> Result<String, String> {
        if self.config.offline {
            return Err("Offline mode enabled".to_string());
        }

        // Check if already pending
        {
            let mut pending = match self.pending.lock() {
                Ok(p) => p,
                Err(_) => return Err("Failed to acquire lock".to_string()),
            };
            if pending.contains(module) {
                return Err("Download already pending".to_string());
            }
            pending.insert(module.to_string());
        }

        let result = download_stub(
            module,
            &self.config.python_version,
            self.config.typeshed_commit.as_deref(),
        );

        // Remove from pending
        if let Ok(mut pending) = self.pending.lock() {
            pending.remove(module);
        }

        if let Ok(ref content) = result {
            // Write to cache on success
            let _ = self.write_cache(module, content);

            // Mark as completed
            if let Ok(mut completed) = self.completed.lock() {
                completed.push(module.to_string());
            }
        }

        result
    }

    /// Start a background download for a module using rayon thread pool
    /// Returns immediately; use `poll_completed` to check for results
    pub fn start_background_download(&self, module: &str)
    where
        Self: Sync,
    {
        if self.config.offline {
            return;
        }

        // Check if already pending
        {
            let mut pending = match self.pending.lock() {
                Ok(p) => p,
                Err(_) => return,
            };
            if pending.contains(module) {
                return;
            }
            pending.insert(module.to_string());
        }

        // For now, just mark the module for download
        // The actual download will happen synchronously when get_or_download is called
        // A full async implementation would use tokio::spawn or rayon here
    }

    /// Poll for completed downloads
    /// Returns module names that have finished downloading
    pub fn poll_completed(&self) -> Vec<String> {
        let mut completed = match self.completed.lock() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        std::mem::take(&mut *completed)
    }

    /// Check if offline mode is enabled
    pub fn is_offline(&self) -> bool {
        self.config.offline
    }

    /// Get the configured typeshed path (for local stubs)
    pub fn typeshed_path(&self) -> Option<&PathBuf> {
        self.config.typeshed_path.as_ref()
    }
}

/// Download a stub from the typeshed GitHub repository
fn download_stub(
    module: &str,
    _python_version: &str,
    commit: Option<&str>,
) -> Result<String, String> {
    // Map module name to typeshed path
    let stub_path = module_to_typeshed_path(module);

    // Build URL
    let branch = commit.unwrap_or("main");
    let url = format!(
        "https://raw.githubusercontent.com/python/typeshed/{}/stdlib/{}",
        branch, stub_path
    );

    // Perform blocking HTTP request
    let response =
        reqwest::blocking::get(&url).map_err(|e| format!("HTTP request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {}: {}", response.status(), url));
    }

    response
        .text()
        .map_err(|e| format!("Failed to read response: {}", e))
}

/// Map a Python module name to its typeshed path
fn module_to_typeshed_path(module: &str) -> String {
    let parts: Vec<&str> = module.split('.').collect();

    if parts.len() == 1 {
        // Single module: could be module.pyi or module/__init__.pyi
        // Try module.pyi first (most common for stdlib)
        format!("{}.pyi", module)
    } else {
        // Nested module like os.path -> os/path.pyi
        format!("{}.pyi", parts.join("/"))
    }
}
