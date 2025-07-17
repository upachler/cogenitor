use std::str::FromStr;

use super::simplepath::SimplePath;

/** Fully Qualified Type Name */
#[derive(PartialEq, PartialOrd, Debug, Clone)]
pub struct FQTN {
    path: SimplePath,
}

impl FQTN {
    pub fn crate_name(&self) -> &str {
        self.path.iter().next().unwrap()
    }

    pub fn module_path(&self) -> Option<&str> {
        let segments: Vec<&str> = self.path.iter().collect();
        if segments.len() > 2 {
            // We need to find the substring that represents the module path
            let full_path = self.path.as_str();
            let crate_name = segments[0];
            let type_name = segments.last().unwrap();

            // Find start position (after crate:: )
            let start = crate_name.len() + 2;
            // Find end position (before ::type)
            let end = full_path.len() - type_name.len() - 2;

            Some(&full_path[start..end])
        } else {
            None
        }
    }

    pub fn module_iter(&self) -> impl Iterator<Item = &str> {
        let segments: Vec<&str> = self.path.iter().collect();
        if segments.len() > 2 {
            let module_segments: Vec<&str> = segments[1..segments.len() - 1].to_vec();
            module_segments.into_iter()
        } else {
            Vec::new().into_iter()
        }
    }

    pub fn type_name(&self) -> &str {
        let segments: Vec<&str> = self.path.iter().collect();
        segments.last().unwrap()
    }

    pub fn builder() -> impl FQTNBuilderCrate {
        FQTNBuilder::default()
    }
}

impl std::fmt::Display for FQTN {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path)
    }
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
        // Parse as SimplePath first
        let path = SimplePath::new(fqtn_candidate).map_err(|e| {
            anyhow::Error::msg(format!(
                "'{fqtn_candidate}' is not a valid rust fully qualified type name: {e}"
            ))
        })?;

        // Validate that we have at least 2 segments (crate::type at minimum)
        let segments: Vec<&str> = path.iter().collect();
        if segments.len() < 2 {
            return Err(anyhow::Error::msg(format!(
                "'{fqtn_candidate}' is not a valid rust fully qualified type name"
            )));
        }

        // Check that none of the segments are keywords
        for segment in &segments {
            Self::check_is_no_keyword(segment, fqtn_candidate)?;
        }

        Ok(FQTN { path })
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

    fn type_name(self, type_name: &str) -> FQTN {
        let mut path_str = self.crate_.unwrap();
        for module in &self.modules {
            path_str.push_str("::");
            path_str.push_str(module);
        }
        path_str.push_str("::");
        path_str.push_str(type_name);

        FQTN {
            path: SimplePath::new(&path_str).expect("Builder should produce valid SimplePath"),
        }
    }
}

#[test]
pub fn test_fqtn_parsing() {
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
            FQTN::from_str(test).is_ok(),
            "{test} should parse successfully"
        );
    }

    for test in invalid_input {
        assert!(FQTN::from_str(test).is_err(), "{test} should fail to parse");
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

#[test]
pub fn test_parse() {
    assert!(FQTN::from_str("Test").is_err());

    let fqtn = FQTN::from_str("crate::Test").unwrap();
    assert_eq!("crate", fqtn.crate_name());
    assert!(fqtn.module_path().is_none());
    assert!(fqtn.module_iter().next().is_none());
    assert_eq!("Test", fqtn.type_name());

    let fqtn = FQTN::from_str("std::string::String").unwrap();
    assert_eq!("std", fqtn.crate_name());
    assert_eq!("string", fqtn.module_path().unwrap());
    assert_eq!("String", fqtn.type_name());
}
