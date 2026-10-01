/// Identity + build provenance of the calling binary. Construct it once in the
/// binary (filling the fields from `env!`/build-script stamps) and pass it to
/// the `run` functions.
///
/// In a real binary the stamps come from `env!`/build-script values; here they
/// are literals so the example compiles standalone:
///
/// ```
/// const TOOL: cli_std::ToolInfo = cli_std::ToolInfo {
///     project: "lumen",                      // env-free; the tool name
///     repo: "faberline/lumen",
///     target: "aarch64-apple-darwin",        // env!("LUMEN_TARGET") in lumen
///     version: env!("CARGO_PKG_VERSION"),
///     git_sha: "unknown",                    // env!("LUMEN_GIT_SHA") in lumen
///     built_at: "unknown",                   // env!("LUMEN_BUILT_AT") in lumen
/// };
/// assert_eq!(TOOL.tag_prefix(), "lumen@");
/// assert_eq!(TOOL.asset_name(), "lumen-aarch64-apple-darwin.tar.gz");
/// ```
#[derive(Clone, Copy, Debug)]
pub struct ToolInfo {
    /// Short tool name — also the release-tag prefix (`<project>@X.Y.Z`), the
    /// asset stem (`<project>-<target>.tar.gz`) and the inner binary name.
    pub project: &'static str,
    /// GitHub `owner/name` that owns releases + the issue tracker.
    pub repo: &'static str,
    /// The exact target triple this binary was built for (build-script stamp).
    pub target: &'static str,
    /// This binary's version (`env!("CARGO_PKG_VERSION")`).
    pub version: &'static str,
    /// Short git sha stamped at build time (or "unknown").
    pub git_sha: &'static str,
    /// Build timestamp stamped at build time (or "unknown").
    pub built_at: &'static str,
}

impl ToolInfo {
    /// Default tracker label for this tool's issue surface.
    pub fn issue_label(&self) -> String {
        format!("app:{}", self.project)
    }

    /// Release-tag prefix, e.g. `lumen@`.
    pub fn tag_prefix(&self) -> String {
        format!("{}@", self.project)
    }

    /// Release-asset filename, e.g. `lumen-aarch64-apple-darwin.tar.gz`.
    pub fn asset_name(&self) -> String {
        format!("{}-{}.tar.gz", self.project, self.target)
    }

    /// Path of the binary inside the release tarball, e.g.
    /// `lumen-aarch64-apple-darwin/lumen`.
    pub fn inner_binary_path(&self) -> String {
        format!("{}-{}/{}", self.project, self.target, self.project)
    }
}
