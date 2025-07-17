//! SimplePath implementation following the Rust reference grammar.
//!
//! Grammar:
//! SimplePath → :: ? SimplePathSegment ( :: SimplePathSegment ) *
//! SimplePathSegment → IDENTIFIER | super | self | crate | $crate

use std::fmt;

/// A validated SimplePath following the Rust reference grammar.
///
/// This newtype wraps a String that has been validated to conform to the
/// SimplePath grammar as defined in the Rust reference.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SimplePath(String);

/// Error type for SimplePath validation failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimplePathError {
    /// The path is empty
    Empty,
    /// Invalid segment found
    InvalidSegment(String),
    /// Invalid character in identifier
    InvalidIdentifier(String),
    /// Path starts with invalid separator
    InvalidStart,
    /// Consecutive separators found
    ConsecutiveSeparators,
}

impl fmt::Display for SimplePathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SimplePathError::Empty => write!(f, "SimplePath cannot be empty"),
            SimplePathError::InvalidSegment(segment) => {
                write!(f, "Invalid path segment: '{}'", segment)
            }
            SimplePathError::InvalidIdentifier(ident) => {
                write!(f, "Invalid identifier: '{}'", ident)
            }
            SimplePathError::InvalidStart => write!(f, "Path cannot start with '::'"),
            SimplePathError::ConsecutiveSeparators => {
                write!(f, "Path cannot contain consecutive '::' separators")
            }
        }
    }
}

impl std::error::Error for SimplePathError {}

/// Iterator over SimplePath segments.
pub struct SimplePathIter<'a> {
    remaining: &'a str,
    started: bool,
}

impl<'a> Iterator for SimplePathIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }

        // Handle leading :: on first iteration
        if !self.started {
            self.started = true;
            if self.remaining.starts_with("::") {
                self.remaining = &self.remaining[2..];
                if self.remaining.is_empty() {
                    return None;
                }
            }
        }

        // Find the next :: separator
        if let Some(pos) = self.remaining.find("::") {
            let segment = &self.remaining[..pos];
            self.remaining = &self.remaining[pos + 2..];
            Some(segment)
        } else {
            // Last segment
            let segment = self.remaining;
            self.remaining = "";
            Some(segment)
        }
    }
}

impl SimplePath {
    /// Creates a new SimplePath after validating the input string.
    ///
    /// # Arguments
    ///
    /// * `path` - The path string to validate
    ///
    /// # Returns
    ///
    /// * `Ok(SimplePath)` - If the path is valid
    /// * `Err(SimplePathError)` - If the path is invalid
    ///
    /// # Examples
    ///
    /// ```
    /// use cogenitor_core::codemodel::simplepath::SimplePath;
    ///
    /// let path = SimplePath::new("std::collections::HashMap").unwrap();
    /// let path2 = SimplePath::new("::std::io").unwrap();
    /// let path3 = SimplePath::new("self::module").unwrap();
    /// let path4 = SimplePath::new("super::parent").unwrap();
    /// let path5 = SimplePath::new("crate::root").unwrap();
    /// ```
    pub fn new(path: &str) -> Result<Self, SimplePathError> {
        if path.is_empty() {
            return Err(SimplePathError::Empty);
        }

        // Check for consecutive separators
        if path.contains("::::") {
            return Err(SimplePathError::ConsecutiveSeparators);
        }

        // Validate each segment
        let mut segments = path.split("::");

        // Handle leading :: case
        let first_segment = segments.next().unwrap();
        if first_segment.is_empty() {
            // Path starts with ::, which is valid
            // Continue with next segment
            if let Some(next_segment) = segments.next() {
                Self::validate_segment(next_segment)?;
            } else {
                return Err(SimplePathError::InvalidStart);
            }
        } else {
            Self::validate_segment(first_segment)?;
        }

        // Validate remaining segments
        for segment in segments {
            if segment.is_empty() {
                return Err(SimplePathError::ConsecutiveSeparators);
            }
            Self::validate_segment(segment)?;
        }

        Ok(SimplePath(path.to_string()))
    }

    /// Validates a single path segment.
    fn validate_segment(segment: &str) -> Result<(), SimplePathError> {
        if segment.is_empty() {
            return Err(SimplePathError::InvalidSegment(segment.to_string()));
        }

        // Check for reserved keywords
        match segment {
            "super" | "self" | "crate" | "$crate" => return Ok(()),
            _ => {}
        }

        // Validate as identifier
        Self::validate_identifier(segment)
    }

    /// Validates an identifier according to Rust rules.
    fn validate_identifier(ident: &str) -> Result<(), SimplePathError> {
        if ident.is_empty() {
            return Err(SimplePathError::InvalidIdentifier(ident.to_string()));
        }

        let mut chars = ident.chars();
        let first = chars.next().unwrap();

        // First character must be a letter or underscore
        if !first.is_alphabetic() && first != '_' {
            return Err(SimplePathError::InvalidIdentifier(ident.to_string()));
        }

        // Remaining characters must be alphanumeric or underscore
        for c in chars {
            if !c.is_alphanumeric() && c != '_' {
                return Err(SimplePathError::InvalidIdentifier(ident.to_string()));
            }
        }

        Ok(())
    }

    /// check if path is global in the sense of
    /// (the rust reference)[https://doc.rust-lang.org/stable/reference/paths.html#r-paths.qualifiers.global-root.intro]
    pub fn is_global(&self) -> bool {
        self.0.as_str().starts_with("::")
    }

    /// Returns an iterator over the path segments.
    ///
    /// # Examples
    ///
    /// ```
    /// use cogenitor_core::codemodel::simplepath::SimplePath;
    ///
    /// let path = SimplePath::new("std::collections::HashMap").unwrap();
    /// let segments: Vec<&str> = path.iter().collect();
    /// assert_eq!(segments, vec!["std", "collections", "HashMap"]);
    ///
    /// let path2 = SimplePath::new("::std::io").unwrap();
    /// let segments2: Vec<&str> = path2.iter().collect();
    /// assert_eq!(segments2, vec!["std", "io"]);
    /// ```
    pub fn iter(&self) -> SimplePathIter<'_> {
        SimplePathIter {
            remaining: &self.0,
            started: false,
        }
    }

    /// Returns the underlying string representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SimplePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for SimplePath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_simple_paths() {
        let test_cases = vec![
            "std",
            "std::collections",
            "std::collections::HashMap",
            "::std",
            "::std::io",
            "self",
            "self::module",
            "super",
            "super::parent",
            "crate",
            "crate::root",
            "$crate",
            "$crate::macro_path",
            "my_module",
            "MyStruct",
            "_private",
            "a1b2c3",
        ];

        for case in test_cases {
            assert!(SimplePath::new(case).is_ok(), "Failed for: {}", case);
        }
    }

    #[test]
    fn test_invalid_simple_paths() {
        let test_cases = vec![
            "",
            "::",
            ":::",
            "std:::collections",
            "std:::",
            "123invalid",
            "invalid-char",
            "invalid.char",
            "invalid char",
        ];

        for case in test_cases {
            assert!(SimplePath::new(case).is_err(), "Should fail for: {}", case);
        }
    }

    #[test]
    fn test_iterator() {
        let path = SimplePath::new("std::collections::HashMap").unwrap();
        let segments: Vec<&str> = path.iter().collect();
        assert_eq!(segments, vec!["std", "collections", "HashMap"]);

        let path2 = SimplePath::new("::std::io").unwrap();
        let segments2: Vec<&str> = path2.iter().collect();
        assert_eq!(segments2, vec!["std", "io"]);

        let path3 = SimplePath::new("single").unwrap();
        let segments3: Vec<&str> = path3.iter().collect();
        assert_eq!(segments3, vec!["single"]);

        let path4 = SimplePath::new("self::module").unwrap();
        let segments4: Vec<&str> = path4.iter().collect();
        assert_eq!(segments4, vec!["self", "module"]);
    }

    #[test]
    fn test_reserved_keywords() {
        let keywords = vec!["super", "self", "crate", "$crate"];
        for keyword in keywords {
            assert!(SimplePath::new(keyword).is_ok());
            assert!(SimplePath::new(&format!("{}::something", keyword)).is_ok());
            assert!(SimplePath::new(&format!("something::{}", keyword)).is_ok());
        }
    }

    #[test]
    pub fn test_is_global() {
        let fqtn = SimplePath::new("std::string::String").unwrap();
        assert!(!fqtn.is_global());

        let fqtn = SimplePath::new("::std::string::String").unwrap();
        assert!(fqtn.is_global());

        let fqtn = SimplePath::new("crate::Test").unwrap();
        assert!(!fqtn.is_global());

        let fqtn = SimplePath::new("::crate::Test").unwrap();
        assert!(fqtn.is_global());
    }
}
