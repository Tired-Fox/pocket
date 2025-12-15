use crate::PocketBaseError;

#[derive(Debug)]
pub enum Error {
    Custom(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
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

impl From<jwt::Error> for Error {
    fn from(value: jwt::Error) -> Self {
        Self::Custom(value.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
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
