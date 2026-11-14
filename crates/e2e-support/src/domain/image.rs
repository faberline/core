/// The `sha256:<64 lower hex>` part of a runtime image id, or `None`.
pub fn normalize_runtime_image_id(raw: &str) -> Option<&str> {
    use super::names::is_lower_hex;
    let rest = match raw.rsplit_once('@') {
        Some((_, after)) => after,
        None => raw
            .strip_prefix("cri-o://")
            .or_else(|| raw.strip_prefix("docker://"))
            .unwrap_or(raw),
    };
    let hex = rest.strip_prefix("sha256:")?;
    is_lower_hex(hex, 64).then_some(rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest() -> String {
        format!("sha256:{}", "a".repeat(64))
    }

    #[test]
    fn strips_runtime_scheme_and_repo() {
        let d = digest();
        assert_eq!(
            normalize_runtime_image_id(&format!("cri-o://{d}")),
            Some(d.as_str())
        );
        assert_eq!(
            normalize_runtime_image_id(&format!("docker://{d}")),
            Some(d.as_str())
        );
        assert_eq!(
            normalize_runtime_image_id(&format!("registry.example.com/team/app@{d}")),
            Some(d.as_str())
        );
        assert_eq!(normalize_runtime_image_id(&d), Some(d.as_str()));
    }

    #[test]
    fn rejects_bad_ids() {
        let upper = format!("cri-o://sha256:{}", "A".repeat(64));
        for raw in ["", "cri-o://sha256:abc", upper.as_str(), "containerd://x"] {
            assert_eq!(normalize_runtime_image_id(raw), None, "{raw}");
        }
    }

    #[test]
    fn a_prefixed_id_is_not_returned_raw() {
        // Negative control: a body that returns the raw input fails here.
        let raw = format!("docker://{}", digest());
        let got = normalize_runtime_image_id(&raw);
        assert_ne!(got, Some(raw.as_str()));
        assert!(got.is_some_and(|s| s.starts_with("sha256:")));
    }
}
