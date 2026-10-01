/// Output format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Markdown,
    Console,
    /// SARIF 2.1.0 for code scanning dashboards (--format sarif)
    Sarif,
    /// GitHub Actions inline annotations (--format github)
    GitHub,
    /// GitLab Code Quality JSON array (--format gitlab)
    GitLab,
    /// Symbol-centric JSON optimized for LLM agent consumption (--format agent)
    Agent,
}

impl OutputFormat {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "json" => Some(OutputFormat::Json),
            "markdown" | "md" => Some(OutputFormat::Markdown),
            "console" | "text" | "default" => Some(OutputFormat::Console),
            "sarif" => Some(OutputFormat::Sarif),
            "github" => Some(OutputFormat::GitHub),
            "gitlab" => Some(OutputFormat::GitLab),
            "agent" => Some(OutputFormat::Agent),
            _ => None,
        }
    }
}
