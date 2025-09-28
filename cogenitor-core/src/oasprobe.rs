use std::io::{BufRead, BufReader, Read};

use regex::Regex;

use crate::adapters::OASMajorVersion;

#[derive(Debug, thiserror::Error)]
pub(super) enum OASProbeError {
    #[error("no OAS version declaration found in input")]
    NoVersionFound,
    #[error("unsupported OAS version '{0}'")]
    UnsupportedVersion(String),
    #[error("error while reading from input")]
    IoError(std::io::Error),
}

const OAS_VERSION_YAML_STR: &str = r"\s*openapi:\s*((\d+\.\d+)\.\d+)";

pub(super) fn probe_yaml_oas_version(input: impl Read) -> Result<OASMajorVersion, OASProbeError> {
    const MAX_PROBE_LINES: usize = 5;
    let mut line_count = 0;
    let input = BufReader::new(input);

    let regex = Regex::new(OAS_VERSION_YAML_STR).unwrap();

    for line_result in input.lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(e) => return Result::Err(OASProbeError::IoError(e)),
        };
        if line_count >= MAX_PROBE_LINES {
            break;
        }
        if let Some(captures) = regex.captures(&line) {
            let major_minor_version = captures.get(2).unwrap().as_str();
            let full_version = captures.get(1).unwrap().as_str();
            let v = match major_minor_version {
                #[cfg(feature = "oas30")]
                "3.0" => OASMajorVersion::OAS30,
                #[cfg(feature = "oas31")]
                "3.1" => OASMajorVersion::OAS31,
                _ => {
                    return Err(OASProbeError::UnsupportedVersion(full_version.to_string()));
                }
            };

            return Ok(v);
        } else {
            line_count += 1;
        }
    }

    return Result::Err(OASProbeError::NoVersionFound);
}

#[cfg(test)]
mod tests {
    use crate::oasprobe::{OASMajorVersion, OASProbeError, probe_yaml_oas_version};

    #[test]
    pub fn test_match() {
        let input = r"
            // leading comment
            openapi: 3.0.3
            "
        .as_bytes();

        let v = probe_yaml_oas_version(input).unwrap();
        assert_eq!(v, OASMajorVersion::OAS30);
    }

    #[test]
    pub fn test_binary() {
        let input = [0u8, 0x0au8, 0xffu8, 0xe0].as_ref();

        match probe_yaml_oas_version(input) {
            Ok(_) => assert!(false, "version should not be recognized in junk"),
            Err(e) => match e {
                OASProbeError::IoError(_) => (), // fine, we expect an IO error
                _ => assert!(false, "expected IoError, got {e:?}"),
            },
        }
    }
    #[test]
    pub fn test_unsupported_version() {
        let mut input = r"
            // leading comment
            openapi: 99.99.99
            "
        .as_bytes();

        match probe_yaml_oas_version(&mut input) {
            Ok(_) => assert!(false, "version should not be recognized in junk"),
            Err(e) => match e {
                OASProbeError::UnsupportedVersion(v) => assert_eq!(v, "99.99.99"), // fine, we expect an IO error
                _ => assert!(false, "expected IoError, got {e:?}"),
            },
        }
    }
}
