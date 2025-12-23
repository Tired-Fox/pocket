use chrono::{DateTime, TimeZone, Utc};
use reqwest::RequestBuilder;
use serde_json::Value;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
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
    fn get(&self, uri: impl AsRef<str>) -> RequestBuilder;
    fn post(&self, uri: impl AsRef<str>) -> RequestBuilder;
    fn patch(&self, uri: impl AsRef<str>) -> RequestBuilder;
    fn delete(&self, uri: impl AsRef<str>) -> RequestBuilder;
}

pub struct Client {
    pub base_uri: Url,
    client: reqwest::Client,
}
impl Client {
    pub fn new(base_uri: impl AsRef<str>) -> Self {
        Self {
            client: Default::default(),
            base_uri: Url::parse(base_uri.as_ref()).unwrap(),
        }
    }

    pub fn authorize(&self, token: Token) -> AuthorizedClient {
        AuthorizedClient::new(self.base_uri.clone(), token)
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
            base_uri: &self.base_uri,
        }
    }

    pub async fn health(&self) -> Result<Health, Error> {
        Ok(self
            .get("/api/health")
            .send()
            .await?
            .json()
            .await?)
    }
}

impl PocketBaseClient for Client {
    fn base_uri(&self) -> String {
        self.base_uri.to_string()
    }

    fn get(&self, uri: impl AsRef<str>) -> RequestBuilder {
        self.client.get(self.base_uri.join(uri.as_ref()).unwrap())
    }

    fn post(&self, uri: impl AsRef<str>) -> RequestBuilder {
        self.client.post(self.base_uri.join(uri.as_ref()).unwrap())
    }

    fn patch(&self, uri: impl AsRef<str>) -> RequestBuilder {
        self.client.patch(self.base_uri.join(uri.as_ref()).unwrap())
    }

    fn delete(&self, uri: impl AsRef<str>) -> RequestBuilder {
        self.client.delete(self.base_uri.join(uri.as_ref()).unwrap())
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

pub struct AuthorizedClient {
    pub base_uri: Url,
    token: Token,
    client: reqwest::Client,
}

impl AuthorizedClient {
    pub fn new(base_url: impl AsRef<str>, token: Token) -> Self {
        Self {
            base_uri: Url::parse(base_url.as_ref()).unwrap(),
            client: Default::default(),
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
            .post(format!("/api/collections/{collection}/auth-refresh"))
            .header("Authorization", auth)
            .send()
            .await?
            .json::<AuthResult>()
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
            base_uri: &self.base_uri,
        }
    }

    pub async fn health(&self) -> Result<Health, Error> {
        Ok(self
            .get("/api/health")
            .send()
            .await?
            .json()
            .await?)
    }
}

impl PocketBaseClient for AuthorizedClient {
    fn base_uri(&self) -> String {
        self.base_uri.to_string()
    }

    fn get(&self, uri: impl AsRef<str>) -> RequestBuilder {
        self.client.get(self.base_uri.join(uri.as_ref()).unwrap())
            .header("Authorization", &self.token.auth)
    }

    fn post(&self, uri: impl AsRef<str>) -> RequestBuilder {
        self.client.post(self.base_uri.join(uri.as_ref()).unwrap())
            .header("Authorization", &self.token.auth)
    }

    fn patch(&self, uri: impl AsRef<str>) -> RequestBuilder {
        self.client.patch(self.base_uri.join(uri.as_ref()).unwrap())
            .header("Authorization", &self.token.auth)
    }

    fn delete(&self, uri: impl AsRef<str>) -> RequestBuilder {
        self.client.delete(self.base_uri.join(uri.as_ref()).unwrap())
            .header("Authorization", &self.token.auth)
    }
}