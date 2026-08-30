//! Issue #18: template + data → JSON document, resolved against a data
//! tree before `Document::from_json` ever sees it. Builds on #17's serde
//! layer (`serde_json::Value` is the JSON model both share).
//!
//! ## ADR: repetition is a JSON construct, not a text marker
//!
//! The issue's own sketch used Handlebars-style text markers
//! (`{{#each items}}...{{/each}}`) spanning across raw JSON source. That
//! was deliberately **not** implemented that way: splicing repeated text
//! into a JSON string before parsing means every substituted value (a
//! customer name with a `"` or `\` in it, say) has to be re-escaped by
//! hand to stay valid JSON — exactly the class of bug this format should
//! make structurally impossible, not merely "usually fine."
//!
//! Instead, both placeholder substitution and repetition operate on the
//! already-parsed `serde_json::Value` tree:
//! - `"{{path.to.value}}"` inside any JSON string is replaced by looking
//!   `path` up in the data tree. A string that *is* exactly one
//!   placeholder (nothing else around it) is replaced by the data value
//!   itself, type and all — `"size": "{{style.big}}"` can resolve to a
//!   JSON number, not just a string. A placeholder embedded in more text
//!   (`"Hello {{name}}"`) always produces a string.
//! - `{"$each": "path.to.array", "template": <value>}`, wherever it
//!   appears as an array element, expands to one copy of `template` per
//!   element of the array at `path`, each with its own element bound as
//!   the innermost scope (its own fields resolve first; anything not
//!   found there falls back to the enclosing scope, so an item template
//!   can still reach document-level data). `{{.}}` refers to the element
//!   itself, for arrays of plain values rather than objects. No
//!   conditions, no expressions, no filters — deliberately, per the issue
//!   ("hier ist die Grenze zur Skriptsprache").

use serde_json::Value;

#[derive(Debug)]
pub enum TemplateError {
    /// `{{path}}` didn't resolve against the data tree (any scope).
    MissingPlaceholder(String),
    /// `$each`'s path didn't resolve to a JSON array.
    InvalidEachTarget(String),
    Json(serde_json::Error),
}

impl std::fmt::Display for TemplateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TemplateError::MissingPlaceholder(path) => write!(f, "missing template placeholder: {{{{{path}}}}}"),
            TemplateError::InvalidEachTarget(path) => write!(f, "$each target {path:?} is not an array"),
            TemplateError::Json(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for TemplateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TemplateError::Json(e) => Some(e),
            _ => None,
        }
    }
}

/// What happens when a `{{path}}` doesn't resolve against the data tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissingPlaceholder {
    /// Fail the whole render — the default: a typo'd or forgotten data
    /// field should surface immediately, not print as a blank in the PDF.
    Error,
    /// Substitute an empty string (whole-placeholder matches become JSON
    /// `null`) instead of failing.
    Empty,
}

/// Resolves `template_json` against `data_json` and returns the result as
/// a JSON string, ready for `Document::from_json`. Both are parsed as
/// plain `serde_json::Value` first — a malformed template or data
/// document fails here with a normal JSON parse error, before any
/// placeholder is even looked at.
pub fn render_template(template_json: &str, data_json: &str, on_missing: MissingPlaceholder) -> Result<String, TemplateError> {
    let template: Value = serde_json::from_str(template_json).map_err(TemplateError::Json)?;
    let data: Value = serde_json::from_str(data_json).map_err(TemplateError::Json)?;
    let resolved = resolve_value(&template, &[&data], on_missing)?;
    serde_json::to_string(&resolved).map_err(TemplateError::Json)
}

/// Looks `path` (dot-separated; numeric segments index into arrays) up in
/// each scope in order, innermost first — the first scope that has it
/// wins, so an `$each` item's own fields shadow the outer data's.
fn resolve_placeholder(path: &str, scopes: &[&Value]) -> Option<Value> {
    if path == "." {
        return scopes.first().map(|v| (*v).clone());
    }
    scopes.iter().find_map(|scope| lookup_path(scope, path)).cloned()
}

fn lookup_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = root;
    for segment in path.split('.') {
        current = match current {
            Value::Object(map) => map.get(segment)?,
            Value::Array(items) => items.get(segment.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(current)
}

/// `Some(path)` if `s` (ignoring surrounding whitespace) is *exactly* one
/// placeholder — the case where the resolved value's own JSON type
/// (number, bool, object, ...) is preserved instead of being stringified.
fn whole_placeholder(s: &str) -> Option<&str> {
    let trimmed = s.trim();
    let inner = trimmed.strip_prefix("{{")?.strip_suffix("}}")?;
    if inner.contains("{{") || inner.contains("}}") {
        return None; // more than one placeholder — fall through to partial substitution
    }
    Some(inner.trim())
}

fn value_to_display_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        Value::Number(_) | Value::Bool(_) => value.to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn resolve_string(s: &str, scopes: &[&Value], on_missing: MissingPlaceholder) -> Result<Value, TemplateError> {
    if let Some(path) = whole_placeholder(s) {
        return match resolve_placeholder(path, scopes) {
            Some(v) => Ok(v),
            None => match on_missing {
                MissingPlaceholder::Error => Err(TemplateError::MissingPlaceholder(path.to_string())),
                MissingPlaceholder::Empty => Ok(Value::Null),
            },
        };
    }

    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            out.push_str(&rest[start..]); // unterminated "{{" — pass through literally
            rest = "";
            break;
        };
        let path = after[..end].trim();
        let value = match resolve_placeholder(path, scopes) {
            Some(v) => v,
            None => match on_missing {
                MissingPlaceholder::Error => return Err(TemplateError::MissingPlaceholder(path.to_string())),
                MissingPlaceholder::Empty => Value::Null,
            },
        };
        out.push_str(&value_to_display_string(&value));
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    Ok(Value::String(out))
}

/// `Some((path, template))` if `item` is exactly an `$each` directive
/// object (`{"$each": "...", "template": ...}`, nothing else) — checked
/// by exact key-set match so a coincidentally-named real field never
/// misfires (`Element`'s own field names never start with `$`).
fn each_directive(item: &Value) -> Option<(&str, &Value)> {
    let obj = item.as_object()?;
    if obj.len() != 2 {
        return None;
    }
    let path = obj.get("$each")?.as_str()?;
    let template = obj.get("template")?;
    Some((path, template))
}

fn resolve_value(node: &Value, scopes: &[&Value], on_missing: MissingPlaceholder) -> Result<Value, TemplateError> {
    match node {
        Value::String(s) => resolve_string(s, scopes, on_missing),
        Value::Array(items) => {
            let mut out = Vec::new();
            for item in items {
                if let Some((path, tmpl)) = each_directive(item) {
                    let target = scopes
                        .iter()
                        .find_map(|s| lookup_path(s, path))
                        .ok_or_else(|| TemplateError::InvalidEachTarget(path.to_string()))?;
                    let Value::Array(elements) = target else {
                        return Err(TemplateError::InvalidEachTarget(path.to_string()));
                    };
                    for element in elements {
                        let mut inner_scopes = Vec::with_capacity(scopes.len() + 1);
                        inner_scopes.push(element);
                        inner_scopes.extend_from_slice(scopes);
                        out.push(resolve_value(tmpl, &inner_scopes, on_missing)?);
                    }
                } else {
                    out.push(resolve_value(item, scopes, on_missing)?);
                }
            }
            Ok(Value::Array(out))
        }
        Value::Object(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            for (k, v) in map {
                out.insert(k.clone(), resolve_value(v, scopes, on_missing)?);
            }
            Ok(Value::Object(out))
        }
        other => Ok(other.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_a_placeholder_embedded_in_text() {
        let out = render_template(
            r#"{"greeting": "Hallo {{customer.name}}, viel Erfolg!"}"#,
            r#"{"customer": {"name": "Frau Musterfrau"}}"#,
            MissingPlaceholder::Error,
        )
        .unwrap();
        assert_eq!(out, r#"{"greeting":"Hallo Frau Musterfrau, viel Erfolg!"}"#);
    }

    #[test]
    fn whole_string_placeholder_preserves_the_data_types_own_json_type() {
        let out = render_template(r#"{"size": "{{size}}"}"#, r#"{"size": 24}"#, MissingPlaceholder::Error).unwrap();
        assert_eq!(out, r#"{"size":24}"#);
    }

    #[test]
    fn missing_placeholder_errors_by_default() {
        let err = render_template(r#"{"x": "{{nope}}"}"#, r#"{}"#, MissingPlaceholder::Error).unwrap_err();
        assert!(matches!(err, TemplateError::MissingPlaceholder(p) if p == "nope"));
    }

    #[test]
    fn missing_placeholder_can_resolve_to_empty_instead() {
        let out = render_template(r#"{"x": "before {{nope}} after"}"#, r#"{}"#, MissingPlaceholder::Empty).unwrap();
        assert_eq!(out, r#"{"x":"before  after"}"#);
    }

    #[test]
    fn each_expands_one_copy_of_the_template_per_array_element() {
        let out = render_template(
            r#"{"rows": [{"$each": "items", "template": {"text": "{{name}}: {{amount}}"}}]}"#,
            r#"{"items": [{"name": "A", "amount": 1}, {"name": "B", "amount": 2}]}"#,
            MissingPlaceholder::Error,
        )
        .unwrap();
        assert_eq!(out, r#"{"rows":[{"text":"A: 1"},{"text":"B: 2"}]}"#);
    }

    #[test]
    fn each_item_fields_shadow_outer_scope_but_fall_back_to_it() {
        let out = render_template(
            r#"{"rows": [{"$each": "items", "template": "{{name}} ({{currency}})"}]}"#,
            r#"{"currency": "EUR", "items": [{"name": "A"}, {"name": "B", "currency": "USD"}]}"#,
            MissingPlaceholder::Error,
        )
        .unwrap();
        assert_eq!(out, r#"{"rows":["A (EUR)","B (USD)"]}"#);
    }

    #[test]
    fn each_target_that_is_not_an_array_is_a_clear_error() {
        let err = render_template(
            r#"{"rows": [{"$each": "items", "template": "x"}]}"#,
            r#"{"items": "not an array"}"#,
            MissingPlaceholder::Error,
        )
        .unwrap_err();
        assert!(matches!(err, TemplateError::InvalidEachTarget(p) if p == "items"));
    }

    #[test]
    fn dotted_path_indexes_into_arrays_by_position() {
        let out = render_template(
            r#"{"first": "{{items.0.name}}"}"#,
            r#"{"items": [{"name": "first item"}, {"name": "second item"}]}"#,
            MissingPlaceholder::Error,
        )
        .unwrap();
        assert_eq!(out, r#"{"first":"first item"}"#);
    }
}
