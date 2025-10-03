use std::{num::ParseIntError, str::FromStr};

use crate::types::StatusSpec;

#[derive(Debug, thiserror::Error)]
pub enum ParseStatusSpecError {
    #[error("parsing status code spec failed")]
    ParseFailed(ParseIntError),
    #[error("status code out of range (not within 100..599")]
    OutOfRange,
}

impl From<ParseIntError> for ParseStatusSpecError {
    fn from(parse_error: ParseIntError) -> Self {
        ParseStatusSpecError::ParseFailed(parse_error)
    }
}

impl FromStr for StatusSpec {
    type Err = ParseStatusSpecError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let status_spec = match s {
            "default" => Self::Default,
            "1XX" => Self::Informational1XX,
            "2XX" => Self::Success2XX,
            "3XX" => Self::Redirection3XX,
            "4XX" => Self::ClientError4XX,
            "5XX" => Self::ServerError5XX,
            s => {
                let code = str::parse::<u16>(s)?;
                match code {
                    100..199 => Self::Informational(code),
                    200..299 => Self::Success(code),
                    300..399 => Self::Redirection(code),
                    400..499 => Self::ClientError(code),
                    500..599 => Self::ServerError(code),
                    _ => return Err(ParseStatusSpecError::OutOfRange),
                }
            }
        };
        Ok(status_spec)
    }
}
