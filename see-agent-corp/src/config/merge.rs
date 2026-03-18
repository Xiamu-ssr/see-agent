use serde_json::Value;

/// Deep merge two JSON values.
///
/// Rules (matching Python implementation):
/// - Both dicts → recurse into children
/// - Otherwise → overlay wins (arrays, scalars, any non-dict)
/// - New keys from overlay are added; base keys not in overlay preserved
pub fn deep_merge(base: &Value, overlay: &Value) -> Value {
    match (base, overlay) {
        (Value::Object(b), Value::Object(o)) => {
            let mut result = b.clone();
            for (key, value) in o {
                if value == &Value::String(String::new()) {
                    continue; // empty string = "not configured", skip
                }
                if let Some(existing) = result.get(key) {
                    result.insert(key.clone(), deep_merge(existing, value));
                } else {
                    result.insert(key.clone(), value.clone());
                }
            }
            Value::Object(result)
        }
        (_, overlay) => overlay.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn merge_flat_objects() {
        let base = json!({"a": 1, "b": 2});
        let overlay = json!({"b": 3, "c": 4});
        let result = deep_merge(&base, &overlay);
        assert_eq!(result, json!({"a": 1, "b": 3, "c": 4}));
    }

    #[test]
    fn merge_nested_objects() {
        let base = json!({"llm": {"model": "gpt-4o", "api_key": "sk-xxx"}});
        let overlay = json!({"llm": {"model": "claude-opus-4-6"}});
        let result = deep_merge(&base, &overlay);
        assert_eq!(
            result,
            json!({"llm": {"model": "claude-opus-4-6", "api_key": "sk-xxx"}})
        );
    }

    #[test]
    fn array_overwrites_entirely() {
        let base = json!({"tools": {"disabled": ["shell", "drag"]}});
        let overlay = json!({"tools": {"disabled": ["screenshot"]}});
        let result = deep_merge(&base, &overlay);
        assert_eq!(result, json!({"tools": {"disabled": ["screenshot"]}}));
    }

    #[test]
    fn mcp_servers_dict_merges() {
        let base = json!({"mcp": {"servers": {"tavily": {"command": "tavily"}}}});
        let overlay = json!({"mcp": {"servers": {"github": {"command": "gh-mcp"}}}});
        let result = deep_merge(&base, &overlay);
        assert_eq!(
            result,
            json!({"mcp": {"servers": {"tavily": {"command": "tavily"}, "github": {"command": "gh-mcp"}}}})
        );
    }

    #[test]
    fn overlay_adds_new_keys() {
        let base = json!({"a": 1});
        let overlay = json!({"b": 2});
        let result = deep_merge(&base, &overlay);
        assert_eq!(result, json!({"a": 1, "b": 2}));
    }

    #[test]
    fn scalar_overlay_wins() {
        let base = json!(42);
        let overlay = json!("hello");
        assert_eq!(deep_merge(&base, &overlay), json!("hello"));
    }

    #[test]
    fn null_overlay_wins() {
        let base = json!({"a": 1});
        let overlay = json!(null);
        assert_eq!(deep_merge(&base, &overlay), json!(null));
    }

    #[test]
    fn empty_string_does_not_override_object() {
        let base = json!({"llm": {"model": "gpt-4o", "api_key": "sk-xxx"}});
        let overlay = json!({"llm": ""});
        let result = deep_merge(&base, &overlay);
        assert_eq!(
            result,
            json!({"llm": {"model": "gpt-4o", "api_key": "sk-xxx"}})
        );
    }

    #[test]
    fn empty_string_does_not_override_array() {
        let base = json!({"tools": {"disabled": ["shell"]}});
        let overlay = json!({"tools": {"disabled": ""}});
        let result = deep_merge(&base, &overlay);
        assert_eq!(result, json!({"tools": {"disabled": ["shell"]}}));
    }

    #[test]
    fn non_empty_string_still_overrides() {
        let base = json!({"name": {"first": "John"}});
        let overlay = json!({"name": "Jane"});
        let result = deep_merge(&base, &overlay);
        assert_eq!(result, json!({"name": "Jane"}));
    }
}
