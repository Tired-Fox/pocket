use reqwest::{Body, multipart::{Form, Part}};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::json;
use tokio_util::codec::{BytesCodec, FramedRead};

use crate::{
    BatchRequest, CreateOptions, Error, PocketBaseError, UpdateOptions, client::PocketBaseClient, files::File,
};

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

        let mut form = Form::new();
        form = form.text(
            "@jsonPayload",
            serde_json::to_string(&json!({ "requests": requests }))?,
        );

        for files in files.into_iter() {
            if let Some(files) = files {
                for (name, file) in files {
                    match file {
                        File::Path(path) => {
                            let file = tokio::fs::File::open(&path).await?;
                            let stream = FramedRead::new(file, BytesCodec::new());

                            form = form
                                .part(
                                    name.to_string(),
                                    Part::stream(Body::wrap_stream(stream))
                                        .file_name(path.file_name().unwrap().to_string_lossy().to_string())
                                        .mime_str(mime_to_ext::ext_to_mime(path.extension().unwrap().to_string_lossy().as_ref()).unwrap())?
                                );
                        },
                        File::Raw {
                            filename,
                            mime,
                            bytes,
                        } => form = form
                            .part(
                                name.to_string(),
                                Part::bytes(bytes.clone())
                                    .file_name(filename.to_string())
                                    .mime_str(&mime)?
                            ),
                    }
                }
            }
        }

        let res = self
            .pocketbase
            .post("/api/batch")
            .multipart(form)
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(res.json::<PocketBaseError>().await?.into());
        }
        Ok(res.json::<T>().await?)
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
        files: impl IntoIterator<Item=(String, File)>,
        options: CreateOptions,
    ) -> Result<(), Error> {
        self.batch.requests.push(BatchRequest::Create {
            collection: self.identifier.to_string(),
            record: serde_json::to_value(record)?,
            files: files.into_iter().collect(),
            options,
        });
        Ok(())
    }

    pub fn update(
        self,
        id: impl std::fmt::Display,
        record: impl Serialize,
        files: impl IntoIterator<Item=(String, File)>,
        options: UpdateOptions,
    ) -> Result<(), Error> {
        self.batch.requests.push(BatchRequest::Update {
            collection: self.identifier.to_string(),
            id: id.to_string(),
            record: serde_json::to_value(record)?,
            files: files.into_iter().collect(),
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
