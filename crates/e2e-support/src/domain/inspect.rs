/// The first element of the JSON array `items` whose `"name"` is `name`.
pub fn named<'a>(items: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
    items.as_array()?.iter().find(|item| has_name(item, name))
}

/// How many elements of `container["env"]` have `"name"` equal to `name`.
pub fn env_count(container: &serde_json::Value, name: &str) -> usize {
    elements(container, "env")
        .filter(|item| has_name(item, name))
        .count()
}

/// The elements of `container["volumeMounts"]` whose `"name"` is `name`.
pub fn mounts_named<'a>(
    container: &'a serde_json::Value,
    name: &str,
) -> Vec<&'a serde_json::Value> {
    elements(container, "volumeMounts")
        .filter(|item| has_name(item, name))
        .collect()
}

/// True when `item["name"]` is the string `name`.
fn has_name(item: &serde_json::Value, name: &str) -> bool {
    item.get("name").and_then(serde_json::Value::as_str) == Some(name)
}

/// The elements of the array at `container[key]`; empty when it is missing or not an array.
fn elements<'a>(
    container: &'a serde_json::Value,
    key: &str,
) -> impl Iterator<Item = &'a serde_json::Value> {
    container
        .get(key)
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn named_returns_the_first_match() {
        let items = json!([{"name": "a", "v": 1}, {"name": "b", "v": 2}, {"name": "b", "v": 3}]);
        assert_eq!(named(&items, "b").expect("found")["v"], 2);
        assert!(named(&items, "z").is_none());
        assert!(named(&json!(null), "a").is_none());
    }

    #[test]
    fn env_and_mount_helpers_count_by_name() {
        let c = json!({
            "env": [{"name": "X"}, {"name": "X"}, {"name": "Y"}],
            "volumeMounts": [{"name": "data"}, {"name": "tmp"}]
        });
        assert_eq!(env_count(&c, "X"), 2);
        assert_eq!(env_count(&c, "Z"), 0);
        assert_eq!(env_count(&json!({}), "X"), 0);
        assert_eq!(mounts_named(&c, "data").len(), 1);
        assert_eq!(mounts_named(&c, "none").len(), 0);
    }
}
