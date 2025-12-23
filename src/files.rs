use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use url::Url;

pub struct FilesBuilder<'c> {
    pub(crate) base_uri: &'c Url,
}

impl<'c> FilesBuilder<'c> {
    pub fn get_url(
        &self,
        collection_id: impl std::fmt::Display,
        id: impl std::fmt::Display,
        filename: impl std::fmt::Display,
    ) -> Url {
        self.base_uri
            .join(&format!("/api/files/{collection_id}/{id}/{filename}"))
            .unwrap()
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum File {
    Path(PathBuf),
    Raw {
        filename: String,
        mime: String,
        bytes: Cow<'static, [u8]>,
    },
}

impl File {
    pub fn path(path: impl AsRef<Path>) -> Self {
        Self::Path(path.as_ref().to_path_buf())
    }

    pub fn raw(
        name: impl std::fmt::Display,
        mime: impl std::fmt::Display,
        bytes: impl Into<Cow<'static, [u8]>>,
    ) -> Self {
        Self::Raw {
            filename: name.to_string(),
            mime: mime.to_string(),
            bytes: bytes.into(),
        }
    }

    // pub(crate) async fn into_form_part(self) -> Result<Part, Error> {
    //     Ok(match self {
    //         Self::Path(path) => Part::file(path).await?,
    //         Self::Raw { mime, bytes, filename: name } => {
    //             Part::bytes(bytes)
    //                 .mime_str(&mime)?
    //                 .file_name(name)
    //         }
    //     })
    // }
}

impl From<String> for File {
    fn from(value: String) -> Self {
        File::Path(PathBuf::from(value))
    }
}

impl From<&str> for File {
    fn from(value: &str) -> Self {
        File::Path(PathBuf::from(value))
    }
}

impl<'a> From<Cow<'a, str>> for File {
    fn from(value: Cow<'a, str>) -> Self {
        File::Path(PathBuf::from(value.as_ref()))
    }
}

impl From<&Path> for File {
    fn from(value: &Path) -> Self {
        File::Path(value.to_path_buf())
    }
}

impl From<PathBuf> for File {
    fn from(value: PathBuf) -> Self {
        File::Path(value)
    }
}

impl<M: std::fmt::Display, N: std::fmt::Display, B: Into<Vec<u8>>> From<(N, M, B)> for File {
    fn from((name, mime, bytes): (N, M, B)) -> Self {
        File::Raw {
            filename: name.to_string(),
            mime: mime.to_string(),
            bytes: Into::<Vec<u8>>::into(bytes).into(),
        }
    }
}
