use std::{collections::BTreeMap, path::PathBuf};

use http_client_multipart::Multipart;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::json;

use crate::{BatchRequest, CreateOptions, Error, PocketBaseError, UpdateOptions, client::PocketBaseClient};

pub struct BatchBuilder<'p, P: PocketBaseClient> {
    pub(crate) pocketbase: &'p P,
    pub(crate) requests: Vec<BatchRequest>,
}

impl<'p, P: PocketBaseClient> BatchBuilder<'p, P> {
    pub fn collection<'c, I: std::fmt::Display>(
        &'c mut self,
        identifier: I,
    ) -> BatchCollectionBuilder<'p, 'c, P, I> {
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

        let mut form = Multipart::new();
        form.add_text(
            "@jsonPayload",
            serde_json::to_string(&json!({ "requests": requests }))?,
        );

        for (i, files) in files.into_iter().enumerate() {
            if let Some(files) = files {
                for (name, path) in files {
                    form.add_file(format!("requests.{i}.{name}"), path, None)
                        .await
                        .map_err(Error::custom)?;
                }
            }
        }

        let res = self
            .pocketbase
            .post("/api/batch")
            .multipart(form)?
            .send_async()
            .await?;

        if !res.status().is_success() {
            return Err(res.json_async::<PocketBaseError>().await?.into());
        }
        Ok(res.json_async::<T>().await?)
    }
}

pub struct BatchCollectionBuilder<'p, 'c, P: PocketBaseClient, I: std::fmt::Display> {
    batch: &'c mut BatchBuilder<'p, P>,
    identifier: I,
}

impl<'p, 'c, P: PocketBaseClient, N> BatchCollectionBuilder<'p, 'c, P, N>
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
