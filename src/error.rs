use std::collections::BTreeMap;

use serde::Deserialize;

use crate::PocketBaseError;

#[derive(Debug, Deserialize)]
pub struct FieldError {
    pub code: String,
    pub message: String,
}

#[derive(Debug)]
pub enum Error {
    Authorization {
        message: String,
        data: BTreeMap<String, FieldError>
    },
    Unauthorized,
    Custom(String),
}
impl Error {
    pub fn custom(value: impl std::fmt::Display) -> Self {
        Self::Custom(value.to_string())
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Authorization { message, data } => {
                writeln!(f, "{message}")?;
                write!(f,
                    "  {}",
                    data.iter()
                        .map(|(name, FieldError { code: _, message })| format!("{name}: {message}"))
                        .collect::<Vec<_>>()
                        .join("\n  ")
                )
            },
            Self::Unauthorized => write!(f, "unauthrized"),
            Self::Custom(value) => f.write_str(value),
        }
    }
}

impl std::error::Error for Error {}

impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        Self::Custom(value.to_string())
    }
}

impl From<jsonwebtoken::errors::Error> for Error {
    fn from(value: jsonwebtoken::errors::Error) -> Self {
        Self::Custom(value.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::Custom(value.to_string())
    }
}

impl From<serde_urlencoded::ser::Error> for Error {
    fn from(value: serde_urlencoded::ser::Error) -> Self {
        Self::Custom(value.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Custom(value.to_string())
    }
}

impl From<PocketBaseError> for Error {
    fn from(value: PocketBaseError) -> Self {
        Self::Custom(value.to_string())
    }
}