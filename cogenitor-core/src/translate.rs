use std::collections::{HashMap, HashSet};

use http::Method;

use crate::types::PathItem;

// Array of strict keywords (currently in use)
const STRICT_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while",
];

// Array of reserved keywords (for future use)
const RESERVED_KEYWORDS: &[&str] = &[
    "abstract", "become", "box", "do", "final", "gen", "macro", "override", "priv", "typeof",
    "unsized", "virtual", "yield",
];

/// Makes first character of the given string uppercase and returns the result
fn capitalize(s: &str) -> String {
    modify_first_char(s, char::to_uppercase)
}

/// Makes first character of the given string lowercase and returns the result
fn decapitalize(s: &str) -> String {
    modify_first_char(s, char::to_lowercase)
}

fn modify_first_char<F, R>(s: &str, m: F) -> String
where
    F: FnOnce(char) -> R,
    R: Iterator<Item = char>,
{
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => m(f).collect::<String>() + c.as_str(),
    }
}
pub(crate) fn schema_to_rust_typename(schema_name: &str) -> String {
    avoid_reserved(&capitalize(schema_name))
}

pub(crate) fn property_to_rust_fieldname(property_name: &str) -> String {
    avoid_reserved(&decapitalize(property_name))
}

pub(crate) fn parameter_to_rust_fn_param(param_name: &str) -> String {
    avoid_reserved(&decapitalize(param_name))
}

fn avoid_reserved(s: &str) -> String {
    if STRICT_KEYWORDS
        .iter()
        .chain(RESERVED_KEYWORDS.iter())
        .any(|e| (*e).eq(s))
    {
        s.to_string() + "_"
    } else {
        s.to_string()
    }
}

/// Turns REST paths defined in an OpenAPI spec into Rust function names.
/// For instance, /foo/bar should become foo_bar(), so '/' is replaced by underscore '_'
/// . Non-Rust identifier characters like '{', '}', '$' will be replaced by '_'
/// as well. Leading and consecutive underscores are eliminated (e.g when converting /foo/{bar}, which would
/// otherwise yield _foo__bar, but are converted to foo_bar)
pub(crate) fn path_method_to_rust_fn_name(
    method: &http::Method,
    path: &str,
) -> anyhow::Result<String> {
    // Convert HTTP method to lowercase
    let method_str = method.as_str().to_lowercase();

    // Clean up the path name
    let cleaned_path = path
        // Remove leading slash if present
        .strip_prefix('/')
        .unwrap_or(path)
        // Replace non-identifier characters with underscores
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        // Convert to lowercase for snake_case
        .to_lowercase()
        // Remove consecutive underscores and leading/trailing underscores
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_");

    // Combine path and method
    let function_name = if cleaned_path.is_empty() {
        method_str
    } else {
        format!("{}_{}", cleaned_path, method_str)
    };

    // Ensure the function name doesn't conflict with Rust keywords
    Ok(avoid_reserved(&function_name))
}

/// converts paths and methods like 'GET /foo/bar' into type names such as
///
pub(crate) fn path_method_to_rust_type_name(method: http::Method, path: &str) -> String {
    let (l, r) = method.as_str().split_at(1);
    let method_str = l.to_uppercase() + &r.to_lowercase();

    let path_rump: String = path
        .strip_prefix("/")
        .unwrap_or(path)
        .split("/")
        .map(|s| {
            s.chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect::<String>()
        })
        .map(|s| capitalize(&s))
        .collect();

    path_rump + &method_str
}

pub(crate) fn media_type_range_to_rust_type_name(media_type_key: &str) -> String {
    media_type_key
        .replace("*", "Any")
        .splitn(2, "/")
        .into_iter()
        .map(|s| capitalize(s))
        .map(|s| s.chars().filter(|c| c.is_alphabetic()).collect::<String>())
        .collect()
}

pub trait ContainsPredicate {
    fn contains_str(&self, item: &str) -> bool;
}
impl ContainsPredicate for Vec<&str> {
    fn contains_str(&self, item: &str) -> bool {
        self.into_iter().any(|e| *e == item)
    }
}
impl ContainsPredicate for HashSet<String> {
    fn contains_str(&self, value: &str) -> bool {
        self.contains(value)
    }
}
impl<V> ContainsPredicate for HashMap<String, V> {
    fn contains_str(&self, item: &str) -> bool {
        self.contains_key(item)
    }
}

/** Implements a collision strategy for generating unique names across a namespace */
pub fn uncollide(predicate: &impl ContainsPredicate, name_candidate: String) -> String {
    let mut n = 0;
    let mut candidate = name_candidate.clone();
    while predicate.contains_str(&candidate) {
        n += 1;
        candidate = format!("{name_candidate}{n}");
    }

    candidate
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::Method;

    #[test]
    fn test_simple_path() {
        let result = path_method_to_rust_fn_name(&Method::GET, "/foo").unwrap();
        assert_eq!(result, "foo_get");
    }

    #[test]
    fn test_path_with_parameters() {
        let result = path_method_to_rust_fn_name(&Method::GET, "/bars/{bar_name}").unwrap();
        assert_eq!(result, "bars_bar_name_get");
    }

    #[test]
    fn test_complex_path() {
        let result =
            path_method_to_rust_fn_name(&Method::POST, "/api/v1/users/{user_id}/posts").unwrap();
        assert_eq!(result, "api_v1_users_user_id_posts_post");
    }

    #[test]
    fn test_different_methods() {
        assert_eq!(
            path_method_to_rust_fn_name(&Method::PUT, "/foo").unwrap(),
            "foo_put"
        );
        assert_eq!(
            path_method_to_rust_fn_name(&Method::DELETE, "/foo").unwrap(),
            "foo_delete"
        );
        assert_eq!(
            path_method_to_rust_fn_name(&Method::PATCH, "/foo").unwrap(),
            "foo_patch"
        );
    }

    #[test]
    fn test_special_characters() {
        let result = path_method_to_rust_fn_name(&Method::GET, "/foo-bar/baz$qux/{param}").unwrap();
        assert_eq!(result, "foo_bar_baz_qux_param_get");
    }

    #[test]
    fn test_root_path() {
        let result = path_method_to_rust_fn_name(&Method::GET, "/").unwrap();
        assert_eq!(result, "get");
    }

    #[test]
    fn test_empty_path() {
        let result = path_method_to_rust_fn_name(&Method::GET, "").unwrap();
        assert_eq!(result, "get");
    }
}
