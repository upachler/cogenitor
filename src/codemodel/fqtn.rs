use std::{fmt, str::FromStr};

use lazy_static::*;
use regex::Regex;

/** Fully Qualified Type Name */
#[derive(PartialEq, PartialOrd, Debug)]
pub struct FQTN {
    crate_name: Box<str>,
    module_path: Option<Box<str>>,
    type_name: Box<str>,
}

impl FQTN {
    pub fn crate_name(&self) -> &str {
        &self.crate_name
    }
    pub fn module_path(&self) -> Option<&str> {
        self.module_path.as_ref().map(|b| b.as_ref())
    }
    pub fn type_name(&self) -> &str {
        &self.type_name
    }
    pub fn builder() -> impl FQTNBuilderCrate {
        FQTNBuilder::default()
    }
}

impl std::fmt::Display for FQTN {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.crate_name())?;
        f.write_str("::")?;
        if let Some(module_path) = self.module_path() {
            f.write_str("::")?;
            f.write_str(module_path)?;
        }
        f.write_str(self.type_name())?;
        Ok(())
    }
}

lazy_static! {
    static ref FQTN_REGEX: Regex = Regex::new(
        r"(?x)
        ^
        ([a-zA-Z_][a-zA-Z0-9_]*)    # group 1: first identifier
                                    # foo::bar::Item -> foo

        (                           # group 2: remaining identifiers and separators
                                    # foo::bar::Item -> ::bar::Item
            (?:::(?:[a-zA-Z_][a-zA-Z0-9_]*))+
        )
        $"
    )
    .expect("error parsing regex");
}

fn is_keyword(s: &str) -> bool {
    match s {
        "pub" | "mod" | "if" | "else" | "use" | "struct" | "trait" | "enum" | "as" | "fn" => true,
        _ => false,
    }
}

impl FQTN {
    fn check_is_no_keyword(item_name: &str, fqtn_candidate: &str) -> Result<(), anyhow::Error> {
        if is_keyword(item_name) {
            Err(anyhow::Error::msg(format!(
                "'{fqtn_candidate}' contains a Rust keyword"
            )))
        } else {
            Ok(())
        }
    }
}

impl<'a> FromStr for FQTN {
    type Err = anyhow::Error;

    fn from_str(fqtn_candidate: &str) -> Result<Self, Self::Err> {
        let captures = FQTN_REGEX.captures(fqtn_candidate).ok_or_else(|| {
            anyhow::Error::msg(format!(
                "'{fqtn_candidate}' is not a valid rust fully qualified type name"
            ))
        })?;

        let crate_name: Box<str> = captures.get(1).unwrap().as_str().to_string().into();
        Self::check_is_no_keyword(&crate_name, fqtn_candidate)?;

        let mut mods_item: Vec<_> = captures
            .get(2)
            .unwrap()
            .as_str()
            .split_at(2)
            .1 // strip '::' at start
            .split("::")
            .collect();

        let type_name: Box<str> = mods_item.remove(mods_item.len() - 1).to_string().into();
        Self::check_is_no_keyword(&type_name, fqtn_candidate)?;

        let module_path = if mods_item.is_empty() {
            None
        } else {
            // check all module names if any of them is a Rust keyword
            mods_item
                .iter()
                .map(|s| Self::check_is_no_keyword(s, fqtn_candidate))
                .filter(|r| r.is_err())
                .next()
                .unwrap_or(Ok(()))?;
            // join all module names together to form a module path string
            Some(mods_item.join("::").into())
        };

        Ok(FQTN {
            crate_name,
            module_path,
            type_name,
        })
    }
}

pub trait FQTNBuilderCrate {
    fn crate_(self, crate_: &str) -> impl FQTNBuilderModOrTypeName;
}

pub trait FQTNBuilderModOrTypeName {
    fn mod_(self, mod_: &str) -> Self;
    fn type_name(self, type_name: &str) -> FQTN;
}

#[derive(Default)]
struct FQTNBuilder {
    crate_: Option<String>,
    modules: Vec<String>,
}

impl FQTNBuilderCrate for FQTNBuilder {
    fn crate_(mut self, crate_: &str) -> impl FQTNBuilderModOrTypeName {
        self.crate_ = Some(crate_.to_string());
        self
    }
}

impl FQTNBuilderModOrTypeName for FQTNBuilder {
    fn mod_(mut self, mod_: &str) -> Self {
        self.modules.push(mod_.to_string());
        self
    }

    fn type_name(mut self, type_name: &str) -> FQTN {
        let module_path = if self.modules.is_empty() {
            None
        } else {
            Some(self.modules.join("::").to_string().into())
        };
        FQTN {
            crate_name: self.crate_.unwrap().to_string().into(),
            module_path,
            type_name: type_name.to_string().into(),
        }
    }
}

#[test]
pub fn test_fqtn_regex() {
    // Example usage
    let valid_input = vec![
        "std::string::String",
        "std::collections::HashMap",
        "_private::module::Type",
        "MyType123::Nested_Type_456",
    ];
    let invalid_input = vec!["single", "::invalid", "invalid::"];

    for test in valid_input {
        assert!(
            FQTN_REGEX.is_match(test),
            "{test} does not match but is expected to"
        );
    }

    for test in invalid_input {
        assert!(
            !FQTN_REGEX.is_match(test),
            "{test} matches but it shouldn't!"
        );
    }
}

#[test]
pub fn test_builders() {
    let fqtn = FQTN::builder().crate_("mycrate").type_name("MyType");
    assert_eq!(fqtn.crate_name(), "mycrate");
    assert_eq!(fqtn.module_path(), None);
    assert_eq!(fqtn.type_name(), "MyType");

    let fqtn = FQTN::builder()
        .crate_("mycrate")
        .mod_("foo")
        .type_name("MyType");
    assert_eq!(fqtn.crate_name(), "mycrate");
    assert_eq!(fqtn.module_path(), Some("foo"));
    assert_eq!(fqtn.type_name(), "MyType");

    let fqtn = FQTN::builder()
        .crate_("mycrate")
        .mod_("foo")
        .mod_("bar")
        .type_name("MyType");
    assert_eq!(fqtn.crate_name(), "mycrate");
    assert_eq!(fqtn.module_path(), Some("foo::bar"));
    assert_eq!(fqtn.type_name(), "MyType");
}
