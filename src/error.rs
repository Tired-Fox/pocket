use isahc::http::uri::InvalidUri;

use crate::PocketBaseError;

#[derive(Debug)]
pub enum Error {
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
            Self::Unauthorized => write!(f, "unauthrized"),
            Self::Custom(value) => f.write_str(value),
        }
    }
}

impl std::error::Error for Error {}

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

impl From<isahc::Error> for Error {
    fn from(value: isahc::Error) -> Self {
        Self::Custom(value.to_string())
    }
}

impl From<isahc::http::Error> for Error {
    fn from(value: isahc::http::Error) -> Self {
        Self::Custom(value.to_string())
    }
}

impl From<InvalidUri> for Error {
    fn from(value: InvalidUri) -> Self {
        Self::Custom(value.to_string())
    }
}
