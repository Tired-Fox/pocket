use std::collections::HashMap;

use reqwest::{Body, Method, header::CONTENT_TYPE, multipart::{Form, Part}};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::json;
use tokio_util::codec::{BytesCodec, FramedRead};

use crate::{
    BatchHttpRequest, BatchRequest, CreateOptions, Error, PocketBaseError, UpdateOptions, client::PocketBaseClient, files::File
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

    pub fn get<'c, I: std::fmt::Display>(
        &'c mut self,
        url: I,
    ) -> BatchHttpBuilder<'p, 'c, P> {
        BatchHttpBuilder::new(self, Method::GET, url.to_string())
    }
    pub fn post<'c, I: std::fmt::Display>(
        &'c mut self,
        url: I,
    ) -> BatchHttpBuilder<'p, 'c, P> {
        BatchHttpBuilder::new(self, Method::POST, url.to_string())
    }
    pub fn put<'c, I: std::fmt::Display>(
        &'c mut self,
        url: I,
    ) -> BatchHttpBuilder<'p, 'c, P> {
        BatchHttpBuilder::new(self, Method::PUT, url.to_string())
    }
    pub fn patch<'c, I: std::fmt::Display>(
        &'c mut self,
        url: I,
    ) -> BatchHttpBuilder<'p, 'c, P> {
        BatchHttpBuilder::new(self, Method::PATCH, url.to_string())
    }
    pub fn delete<'c, I: std::fmt::Display>(
        &'c mut self,
        url: I,
    ) -> BatchHttpBuilder<'p, 'c, P> {
        BatchHttpBuilder::new(self, Method::DELETE, url.to_string())
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

        for (i, files) in files.into_iter().enumerate() {
            if let Some(files) = files {
                for (name, file) in files {
                    match file {
                        File::Path(path) => {
                            let file = tokio::fs::File::open(&path).await?;
                            let stream = FramedRead::new(file, BytesCodec::new());

                            form = form
                                .part(
                                    format!("requests.{i}.{name}"),
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
                                format!("requests.{i}.{name}"),
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

pub struct BatchHttpBuilder<'p, 'c, P: PocketBaseClient> {
    batch: &'c mut BatchBuilder<'p, P>,

    method: Method,
    url: String,
    headers: HashMap<String, String>,
    query: Option<String>,
    body: Option<String>,
}
impl<'p, 'c, P: PocketBaseClient> BatchHttpBuilder<'p, 'c, P> {
    pub(crate) fn new(batch: &'c mut BatchBuilder<'p, P>, method: Method, url: String) -> Self {
        Self {
            batch,
            method,
            url,
            headers: Default::default(),
            query: None,
            body: None,
        }
    }

    pub fn header<K, V>(mut self, key: K, value: V) -> Self
    where
        K: std::fmt::Display,
        V: std::fmt::Display,
    {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    pub fn query<S: Serialize>(self, query: S) -> Result<Self, serde_urlencoded::ser::Error> {
        Ok(Self {
            query: Some(serde_urlencoded::to_string(query)?),
            ..self
        })
    }

    pub fn json<S: Serialize>(mut self, body: &S) -> Result<Self, serde_json::Error> {
        self.body = Some(serde_json::to_string(body)?);
        Ok(self.header(CONTENT_TYPE, "application/json"))
    }

    pub fn finish(self) {
        self.batch.requests.push(BatchRequest::Http(BatchHttpRequest {
            method: self.method.to_string(),
            url: if let Some(q) = self.query {
                format!("{}?{q}", self.url)
            } else {
                self.url
            },
            headers: self.headers,
            body: self.body
        }));
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
