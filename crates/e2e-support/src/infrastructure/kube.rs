//! `kubectl` in one namespace, and a dump of cluster state on a failed test.

use super::step::say;

/// `kubectl` in one namespace.
#[derive(Debug, Clone)]
pub struct Kube {
    /// The namespace that every command uses.
    pub namespace: String,
}

impl Kube {
    /// A client for the namespace of the run (see `env::namespace`).
    pub fn from_env() -> Self {
        Self {
            namespace: super::env::namespace(),
        }
    }

    /// Runs `kubectl -n <namespace> <args>` and returns its stdout.
    pub fn try_run(&self, args: &[&str]) -> Result<String, String> {
        run_command("kubectl", &kubectl_args(&self.namespace, args))
    }

    /// Like `try_run`, but panics on failure.
    ///
    /// # Panics
    ///
    /// Panics when kubectl fails.
    pub fn run(&self, args: &[&str]) -> String {
        self.try_run(args)
            .unwrap_or_else(|e| panic!("kubectl {} failed: {e}", args.join(" ")))
    }

    /// The JSON of one object, for example `pod/x`.
    pub fn get(&self, what: &str) -> serde_json::Value {
        let text = self.run(&["get", what, "-o", "json"]);
        parse_json(what, &text)
    }

    /// Whether the object exists.
    pub fn exists(&self, what: &str) -> bool {
        self.try_run(&["get", what, "-o", "name"]).is_ok()
    }

    /// The pods with `label`, as JSON.
    ///
    /// # Panics
    ///
    /// Panics when kubectl fails or prints invalid JSON.
    pub fn pods(&self, label: &str) -> serde_json::Value {
        self.try_pods(label).unwrap_or_else(|e| panic!("{e}"))
    }

    pub(crate) fn try_pods(&self, label: &str) -> Result<serde_json::Value, String> {
        let text = self.try_run(&["get", "pods", "-l", label, "-o", "json"])?;
        serde_json::from_str(&text).map_err(|e| format!("invalid JSON from kubectl for pods: {e}"))
    }
}

/// Dumps cluster state when a test panics. The commands run only while
/// the thread is unwinding.
pub struct DumpOnFailure<'a> {
    /// The suite name used in the dump lines.
    pub suite: &'static str,
    /// The client that runs the commands.
    pub kube: &'a Kube,
    /// Each command as its arguments, without `kubectl -n <ns>`.
    pub dump_commands: Vec<Vec<String>>,
}

impl Drop for DumpOnFailure<'_> {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            return;
        }
        for command in &self.dump_commands {
            let refs: Vec<&str> = command.iter().map(String::as_str).collect();
            let result = self.kube.try_run(&refs);
            say(dump_report(self.suite, command, &result));
        }
    }
}

fn dump_report(suite: &str, command: &[String], result: &Result<String, String>) -> String {
    let body = match result {
        Ok(out) => out.clone(),
        Err(e) => format!("error: {e}"),
    };
    format!("dump {suite}: kubectl {}\n{body}", command.join(" "))
}

fn kubectl_args(namespace: &str, args: &[&str]) -> Vec<String> {
    let mut out = vec!["-n".to_string(), namespace.to_string()];
    out.extend(args.iter().map(|a| a.to_string()));
    out
}

fn run_command(program: &str, args: &[String]) -> Result<String, String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("{program} failed to start: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn parse_json(what: &str, text: &str) -> serde_json::Value {
    serde_json::from_str(text)
        .unwrap_or_else(|e| panic!("invalid JSON from kubectl for {what}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kubectl_args_put_the_namespace_first() {
        assert_eq!(
            kubectl_args("ns", &["get", "pods"]),
            vec![
                "-n".to_string(),
                "ns".to_string(),
                "get".to_string(),
                "pods".to_string()
            ]
        );
    }

    #[test]
    fn run_command_returns_stdout() {
        assert_eq!(
            run_command("sh", &["-c".into(), "printf hi".into()]),
            Ok("hi".to_string())
        );
    }

    #[test]
    fn run_command_returns_trimmed_stderr_on_failure() {
        assert_eq!(
            run_command("sh", &["-c".into(), "echo bad >&2; exit 3".into()]),
            Err("bad".to_string())
        );
    }

    #[test]
    fn run_command_reports_a_missing_program() {
        let err = run_command("e2e-support-no-such-program", &[]).unwrap_err();
        assert!(err.contains("failed to start"), "{err}");
    }

    #[test]
    fn parse_json_parses_a_document() {
        assert_eq!(parse_json("pod/x", "{\"a\":1}")["a"], 1);
    }

    #[test]
    #[should_panic(expected = "invalid JSON from kubectl for pod/x")]
    fn parse_json_panics_on_garbage() {
        parse_json("pod/x", "not json");
    }

    #[test]
    fn dump_report_shows_command_and_output() {
        assert_eq!(
            dump_report("s", &["get".into(), "pods".into()], &Ok("out".into())),
            "dump s: kubectl get pods\nout"
        );
    }

    #[test]
    fn dump_report_shows_the_error() {
        let text = dump_report("s", &["get".into(), "pods".into()], &Err("boom".into()));
        assert!(text.ends_with("error: boom"), "{text}");
    }
}
