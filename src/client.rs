use chrono::{DateTime, TimeZone, Utc};
use futures_lite::io::AsyncRead;
use serde_json::Value;
use std::{collections::BTreeMap, io::Read};

use http_client_multipart::Multipart;
use isahc::{
    AsyncBody, AsyncReadResponseExt, Body, ReadResponseExt,
    http::{
        Extensions, HeaderMap, HeaderName, HeaderValue, StatusCode, Version, request::Builder,
        uri::PathAndQuery,
    },
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use url::Url;

use crate::{Claims, Error, FilesBuilder, Health, batch::BatchBuilder, collection::CollectionBuilder, error::FieldError};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Token {
    pub collection: String,
    pub user: String,
    pub auth: String,
    pub expires: DateTime<Utc>,
    pub refreshable: bool,
    pub ty: String,
}
impl Token {
    pub fn is_expired(&self) -> bool {
        self.expires < Utc::now()
    }
}

pub trait PocketBaseClient {
    fn base_uri(&self) -> String;
    fn get(&self, uri: impl AsRef<str>) -> RequestBuilder<'_>;
    fn post(&self, uri: impl AsRef<str>) -> RequestBuilder<'_>;
    fn patch(&self, uri: impl AsRef<str>) -> RequestBuilder<'_>;
    fn delete(&self, uri: impl AsRef<str>) -> RequestBuilder<'_>;
}

#[derive(Clone)]
pub struct Client {
    pub base_url: Url,
    client: isahc::HttpClient,
}

impl Client {
    pub fn new(base_url: impl AsRef<str>) -> Self {
        Self {
            client: isahc::HttpClient::new().unwrap(),
            base_url: Url::parse(base_url.as_ref()).unwrap(),
        }
    }

    pub fn authorize(&self, token: Token) -> AuthorizedClient {
        AuthorizedClient { client: self.clone(), token }
    }

    pub fn collection<'c, I: std::fmt::Display>(
        &'c self,
        identifier: I,
    ) -> CollectionBuilder<'c, Self, I> {
        CollectionBuilder {
            pocketbase: self,
            identifier,
        }
    }

    pub fn create_batch<'c>(&'c self) -> BatchBuilder<'c, Self> {
        BatchBuilder {
            pocketbase: self,
            requests: Default::default(),
        }
    }

    pub fn files<'c>(&'c self) -> FilesBuilder<'c> {
        FilesBuilder {
            base_url: &self.base_url,
        }
    }

    pub async fn health(&self) -> Result<Health, Error> {
        Ok(self
            .get("/api/health")
            .send_async()
            .await?
            .json_async()
            .await?)
    }
}

impl PocketBaseClient for Client {
    fn base_uri(&self) -> String {
        self.base_url.to_string()
    }

    fn get(&self, uri: impl AsRef<str>) -> RequestBuilder<'_> {
        RequestBuilder {
            client: &self.client,
            request_builder: isahc::Request::get(
                self.base_url.join(uri.as_ref()).unwrap().to_string(),
            ),
        }
    }

    fn post(&self, uri: impl AsRef<str>) -> RequestBuilder<'_> {
        RequestBuilder {
            client: &self.client,
            request_builder: isahc::Request::post(
                self.base_url.join(uri.as_ref()).unwrap().to_string(),
            ),
        }
    }

    fn patch(&self, uri: impl AsRef<str>) -> RequestBuilder<'_> {
        RequestBuilder {
            client: &self.client,
            request_builder: isahc::Request::patch(
                self.base_url.join(uri.as_ref()).unwrap().to_string(),
            ),
        }
    }

    fn delete(&self, uri: impl AsRef<str>) -> RequestBuilder<'_> {
        RequestBuilder {
            client: &self.client,
            request_builder: isahc::Request::delete(
                self.base_url.join(uri.as_ref()).unwrap().to_string(),
            ),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum AuthResult {
    Error {
        status: u16,
        #[serde(default)]
        message: Option<String>,
        #[serde(default)]
        data: BTreeMap<String, FieldError>,
    },
    Success {
        token: String,
        record: Value,
    },
}

#[derive(Clone)]
pub struct AuthorizedClient {
    client: Client,
    token: Token,
}

impl AuthorizedClient {
    pub fn new(base_url: impl AsRef<str>, token: Token) -> Self {
        Self {
            client: Client::new(base_url),
            token
        }
    }

    pub fn token(self) -> Token {
        self.token
    }

    pub fn is_expired(&self) -> bool {
        self.token.is_expired()
    }

    pub async fn refresh(&mut self) -> Result<(), Error> {
        let Token {
            auth, collection, ..
        } = &self.token;

        let result = self
            .client
            .post(format!("/api/collections/{collection}/auth-refresh"))
            .header("Authorization", auth)
            .send_async()
            .await?
            .json_async::<AuthResult>()
            .await?;

        match result {
            AuthResult::Error { message, .. } => {
                return Err(Error::Custom(
                    message.unwrap_or("failed to authenticate user".into()),
                ));
            }
            AuthResult::Success { token, record } => {
                let claims = unsafe { Claims::decode_unsafe(&token)? };
                self.token = Token {
                    user: record.as_object().unwrap().get("id").unwrap().as_str().unwrap().to_string(),
                    collection: collection.clone(),
                    auth: token,
                    refreshable: claims.refreshable,
                    ty: claims.ty,
                    expires: Utc.timestamp_opt(claims.exp, 0).unwrap(),
                };
            }
        }

        Ok(())
    }

    pub fn collection<'c, I: std::fmt::Display>(
        &'c self,
        identifier: I,
    ) -> CollectionBuilder<'c, Self, I> {
        CollectionBuilder {
            pocketbase: self,
            identifier,
        }
    }

    pub fn create_batch<'c>(&'c self) -> BatchBuilder<'c, Self> {
        BatchBuilder {
            pocketbase: self,
            requests: Default::default(),
        }
    }

    pub fn files<'c>(&'c self) -> FilesBuilder<'c> {
        FilesBuilder {
            base_url: &self.client.base_url,
        }
    }

    pub async fn health(&self) -> Result<Health, Error> {
        Ok(self
            .get("/api/health")
            .send_async()
            .await?
            .json_async()
            .await?)
    }
}

impl PocketBaseClient for AuthorizedClient {
    fn base_uri(&self) -> String {
        self.client.base_uri()
    }

    fn get(&self, uri: impl AsRef<str>) -> RequestBuilder<'_> {
        self.client.get(uri)
            .header("Authorization", &self.token.auth)
    }

    fn post(&self, uri: impl AsRef<str>) -> RequestBuilder<'_> {
        self.client.post(uri)
            .header("Authorization", &self.token.auth)
    }

    fn patch(&self, uri: impl AsRef<str>) -> RequestBuilder<'_> {
        self.client.patch(uri)
            .header("Authorization", &self.token.auth)
    }

    fn delete(&self, uri: impl AsRef<str>) -> RequestBuilder<'_> {
        self.client.delete(uri)
            .header("Authorization", &self.token.auth)
    }
}

pub struct RequestBuilder<'c> {
    client: &'c isahc::HttpClient,
    request_builder: Builder,
}

#[allow(dead_code)]
impl<'c> RequestBuilder<'c> {
    pub fn header<K, V>(self, key: K, value: V) -> Self
    where
        HeaderName: TryFrom<K>,
        <HeaderName as TryFrom<K>>::Error: Into<isahc::http::Error>,
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: Into<isahc::http::Error>,
    {
        Self {
            client: self.client,
            request_builder: self.request_builder.header(key, value),
        }
    }

    pub fn query<S: Serialize>(self, value: S) -> Result<Self, Error> {
        let query = serde_urlencoded::to_string(value)?;

        let mut parts = self.request_builder.uri_ref().unwrap().clone().into_parts();

        let path_and_query = parts.path_and_query.as_ref().unwrap();
        let path_and_query =
            PathAndQuery::try_from(format!("{}?{}", path_and_query.path(), query))?;

        parts.path_and_query.replace(path_and_query);

        Ok(Self {
            client: self.client,
            request_builder: self.request_builder.uri(parts),
        })
    }

    pub fn multipart(self, form: Multipart) -> Result<Request<'c, AsyncBody>, Error> {
        Ok(self
            .header("Content-Type", "multipart/form-data")
            .body(AsyncBody::from_reader(form.into_reader(None)))?)
    }

    pub fn json<T: Serialize>(self, data: &T) -> Result<Request<'c, String>, Error> {
        Ok(self
            .header("Content-Type", "application/json")
            .body(serde_json::to_string(data)?)?)
    }

    pub fn form<T: Serialize>(self, data: &T) -> Result<Request<'c, String>, Error> {
        Ok(self
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(serde_urlencoded::to_string(data)?)?)
    }

    pub fn body<B>(self, body: B) -> Result<Request<'c, B>, Error> {
        Ok(Request {
            client: self.client,
            request: self.request_builder.body(body)?,
        })
    }

    pub fn send(self) -> Result<Response<Body>, Error> {
        Request {
            client: self.client,
            request: self.request_builder.body(())?,
        }
        .send()
    }

    pub async fn send_async(self) -> Result<Response<AsyncBody>, Error> {
        Request {
            client: self.client,
            request: self.request_builder.body(())?,
        }
        .send_async()
        .await
    }
}

pub struct Request<'c, B> {
    client: &'c isahc::HttpClient,
    request: isahc::Request<B>,
}

impl<'c, B: Into<Body>> Request<'c, B> {
    pub fn send(self) -> Result<Response<Body>, Error> {
        Ok(Response(self.client.send(self.request)?))
    }
}

impl<'c, B: Into<AsyncBody>> Request<'c, B> {
    pub async fn send_async(self) -> Result<Response<AsyncBody>, Error> {
        Ok(Response(self.client.send_async(self.request).await?))
    }
}

pub struct Response<B>(isahc::Response<B>);
#[allow(dead_code)]
impl<R> Response<R> {
    pub fn version(&self) -> Version {
        self.0.version()
    }

    pub fn status(&self) -> StatusCode {
        self.0.status()
    }

    pub fn headers(&self) -> &HeaderMap<HeaderValue> {
        self.0.headers()
    }

    pub fn extensions(&self) -> &Extensions {
        self.0.extensions()
    }
}

#[allow(dead_code)]
impl<R: Read> Response<R> {
    pub fn text(mut self) -> std::io::Result<String> {
        self.0.text()
    }

    pub fn json<T: DeserializeOwned>(mut self) -> std::io::Result<T> {
        let body = self.0.text()?;
        Ok(serde_json::from_str(&body)?)
    }
}

#[allow(dead_code)]
impl<R: AsyncRead + Unpin> Response<R> {
    pub async fn text_async(mut self) -> std::io::Result<String> {
        self.0.text().await
    }

    pub async fn json_async<T: DeserializeOwned>(mut self) -> std::io::Result<T> {
        let body = self.0.text().await?;
        Ok(serde_json::from_str(&body)?)
    }
}
