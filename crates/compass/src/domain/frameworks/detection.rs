use std::collections::HashMap;
use std::path::PathBuf;

// ============================================================================
// Framework Detection
// ============================================================================

/// Detected framework in a project.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Framework {
    Django,
    Flask,
    FastAPI,
    Pydantic,
    SQLAlchemy,
    Celery,
    Custom(String),
}

/// Framework detection result.
#[derive(Debug, Clone)]
pub struct FrameworkDetection {
    /// Detected frameworks
    pub frameworks: Vec<Framework>,
    /// Framework-specific files
    pub framework_files: HashMap<Framework, Vec<PathBuf>>,
    /// Confidence scores (0.0 to 1.0)
    pub confidence: HashMap<Framework, f64>,
}

impl FrameworkDetection {
    /// Create empty detection.
    pub fn empty() -> Self {
        Self {
            frameworks: Vec::new(),
            framework_files: HashMap::new(),
            confidence: HashMap::new(),
        }
    }

    /// Check if a framework was detected.
    pub fn has_framework(&self, framework: &Framework) -> bool {
        self.frameworks.contains(framework)
    }

    /// Get confidence for a framework.
    pub fn confidence_for(&self, framework: &Framework) -> f64 {
        self.confidence.get(framework).copied().unwrap_or(0.0)
    }

    /// Add a detected framework with confidence.
    ///
    /// If the framework was already detected, takes the maximum confidence.
    pub fn add_framework(&mut self, framework: Framework, confidence: f64) {
        if !self.frameworks.contains(&framework) {
            self.frameworks.push(framework.clone());
        }

        // Take maximum of existing and new confidence
        let current_confidence = self.confidence.get(&framework).copied().unwrap_or(0.0);
        let max_confidence = current_confidence.max(confidence);
        self.confidence.insert(framework, max_confidence);
    }
}
