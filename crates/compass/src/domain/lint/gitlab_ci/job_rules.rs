use std::collections::HashSet;

use super::{CiJob, GitlabCiChecker};
use crate::diagnostic::{Diagnostic, DiagnosticCategory, Position, Range};

impl GitlabCiChecker {
    /// GL003: Invalid stage reference
    pub(super) fn check_invalid_stages(
        stages: &[String],
        jobs: &[CiJob],
        lines: &[&str],
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        if stages.is_empty() {
            return diagnostics;
        }

        let valid_stages: HashSet<&str> = stages.iter().map(|s| s.as_str()).collect();
        for job in jobs {
            if let Some(ref stage) = job.stage {
                if !valid_stages.contains(stage.as_str()) {
                    let line = job.start_line;
                    let col = lines.get(line).map(|l| l.len()).unwrap_or(0);
                    diagnostics.push(Diagnostic::error(
                        Range::new(
                            Position::new(line as u32, 0),
                            Position::new(line as u32, col as u32),
                        ),
                        "GL003",
                        DiagnosticCategory::Logic,
                        format!(
                            "Job '{}' references undefined stage '{}' — define it in 'stages:'",
                            job.name, stage,
                        ),
                    ));
                }
            }
        }
        diagnostics
    }

    /// GL004: Missing script in job
    pub(super) fn check_missing_script(jobs: &[CiJob], lines: &[&str]) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for job in jobs {
            if !job.has_script {
                let col = lines.get(job.start_line).map(|l| l.len()).unwrap_or(0);
                diagnostics.push(Diagnostic::error(
                    Range::new(
                        Position::new(job.start_line as u32, 0),
                        Position::new(job.start_line as u32, col as u32),
                    ),
                    "GL004",
                    DiagnosticCategory::Logic,
                    format!("Job '{}' is missing a 'script' key", job.name),
                ));
            }
        }
        diagnostics
    }

    /// GL007: Mixing `rules` with `only`/`except`
    pub(super) fn check_rules_only_mixed(jobs: &[CiJob], lines: &[&str]) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for job in jobs {
            if job.has_rules && (job.has_only || job.has_except) {
                let col = lines.get(job.start_line).map(|l| l.len()).unwrap_or(0);
                diagnostics.push(Diagnostic::warning(
                    Range::new(
                        Position::new(job.start_line as u32, 0),
                        Position::new(job.start_line as u32, col as u32),
                    ),
                    "GL007",
                    DiagnosticCategory::Logic,
                    format!(
                        "Job '{}' mixes 'rules' with 'only/except' — use 'rules' exclusively",
                        job.name,
                    ),
                ));
            }
        }
        diagnostics
    }
}
