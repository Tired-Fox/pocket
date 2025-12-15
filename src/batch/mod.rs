use std::{collections::BTreeMap, path::PathBuf};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    collection::{CreateOptions, UpdateOptions}, Error, ExtendAuth, PocketBase, PocketBaseError
};

pub enum BatchRequest {
    Create {
        collection: String,
        record: Value,
        files: BTreeMap<String, PathBuf>,
        options: CreateOptions,
    },
    Update {
        collection: String,
        id: String,
        record: Value,
        files: BTreeMap<String, PathBuf>,
        options: UpdateOptions,
    },
    Delete {
        collection: String,
        id: String,
    },
}

impl BatchRequest {
    pub fn request(&self) -> Value {
        match self {
            Self::Create {
                collection,
                record,
                options,
                ..
            } => {
                let query = serde_urlencoded::to_string(options).unwrap_or_default();
                let url = if query.is_empty() {
                    format!("/api/collections/{collection}/records")
                } else {
                    format!("/api/collections/{collection}/records?{}", query)
                };
                json!({
                    "method": "POST",
                    "url": url,
                    "body": record
                })
            }
            Self::Update {
                collection,
                id,
                record,
                options,
                ..
            } => {
                let query = serde_urlencoded::to_string(options).unwrap_or_default();
                let url = if query.is_empty() {
                    format!("/api/collections/{collection}/records/{id}")
                } else {
                    format!("/api/collections/{collection}/records/{id}?{}", query)
                };
                json!({
                    "method": "PATCH",
                    "url": url,
                    "body": record
                })
            }
            Self::Delete { collection, id } => json!({
                "method": "DELETE",
                "url": format!("/api/collections/{collection}/records/{id}"),
            }),
        }
    }

    pub fn files(&self) -> Option<&BTreeMap<String, PathBuf>> {
        match self {
            Self::Create { files, .. } => (!files.is_empty()).then_some(files),
            Self::Update { files, .. } => (!files.is_empty()).then_some(files),
            Self::Delete { .. } => None,
        }
    }
}

#[derive(Deserialize)]
pub struct BatchResult<T = Value> {
    pub status: u16,
    pub body: T,
}

impl<T: std::fmt::Debug> std::fmt::Debug for BatchResult<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BatchResult")
            .field("status", &self.status)
            .field("body", &self.body)
            .finish()
    }
}

pub struct BatchBuilder<'p> {
    pub(crate) pocketbase: &'p mut PocketBase,
    pub(crate) requests: Vec<BatchRequest>,
}

impl<'p> BatchBuilder<'p> {
    pub fn collection<'c, I: std::fmt::Display>(
        &'c mut self,
        identifier: I,
    ) -> BatchCollectionBuilder<'p, 'c, I> {
        BatchCollectionBuilder {
            batch: self,
            identifier,
        }
    }

    pub async fn send<T: DeserializeOwned>(self) -> Result<T, Error> {
        let (requests, files) =
            self.requests
                .iter()
                .fold((Vec::new(), Vec::new()), |mut ctx, request| {
                    ctx.0.push(request.request());
                    ctx.1.push(request.files());
                    ctx
                });

        let mut form = reqwest::multipart::Form::new().text(
            "@jsonPayload",
            serde_json::to_string(&json!({ "requests": requests }))?,
        );

        for (i, files) in files.into_iter().enumerate() {
            if let Some(files) = files {
                for (name, path) in files {
                    form = form.file(format!("requests.{i}.{name}"), path).await?;
                }
            }
        }

        let res = self.pocketbase.client
            .post(format!("{}/api/batch", self.pocketbase.base_uri))
            .auth(self.pocketbase)
            .await?
            .multipart(form)
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(res.json::<PocketBaseError>().await?.into());
        }
        Ok(res.json::<T>().await?)
    }
}

pub struct BatchCollectionBuilder<'p, 'c, I: std::fmt::Display> {
    batch: &'c mut BatchBuilder<'p>,
    identifier: I,
}

impl<'p, 'c, N> BatchCollectionBuilder<'p, 'c, N>
where
    N: std::fmt::Display,
{
    pub fn create(
        self,
        record: impl Serialize,
        files: impl Into<BTreeMap<String, PathBuf>>,
        options: CreateOptions,
    ) -> Result<(), Error> {
        self.batch.requests.push(BatchRequest::Create {
            collection: self.identifier.to_string(),
            record: serde_json::to_value(record)?,
            files: files.into(),
            options,
        });
        Ok(())
    }

    pub fn update(
        self,
        id: impl std::fmt::Display,
        record: impl Serialize,
        files: impl Into<BTreeMap<String, PathBuf>>,
        options: UpdateOptions,
    ) -> Result<(), Error> {
        self.batch.requests.push(BatchRequest::Update {
            collection: self.identifier.to_string(),
            id: id.to_string(),
            record: serde_json::to_value(record)?,
            files: files.into(),
            options,
        });
        Ok(())
    }

    pub fn delete(self, id: impl std::fmt::Display) {
        self.batch.requests.push(BatchRequest::Delete {
            collection: self.identifier.to_string(),
            id: id.to_string(),
        });
    }
}
