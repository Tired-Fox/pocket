use chrono::{DateTime, TimeZone, Utc};
use reqwest::{Client as HttpClient, RequestBuilder};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type Record = serde_json::Map<String, Value>;

mod error;
pub use error::Error;

pub mod batch;
use batch::BatchBuilder;

pub mod collection;
use collection::CollectionBuilder;

pub mod files;
use files::FilesBuilder;

pub(crate) trait ExtendAuth: Sized {
    fn auth(self, instance: &mut PocketBase) -> impl Future<Output = Result<Self, Error>> + Send;
}
impl ExtendAuth for RequestBuilder {
    async fn auth(self, instance: &mut PocketBase) -> Result<Self, Error> {
        if instance.token.is_some() && !instance.is_valid() {
            instance.auth_refresh().await?;
        }

        Ok(if let Some(Token { auth, .. }) = instance.token.as_ref() {
            self.header("Authorization", auth)
        } else {
            self
        })
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
        data: Option<Value>,
    },
    Success {
        token: String,
    },
}

#[derive(Debug, Deserialize)]
struct PocketBaseError{
    status: u16,
    message: String,
    data: Value,
}
impl std::fmt::Display for PocketBaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f,
            "[{}] {}: {}",
            self.status,
            self.message,
            serde_json::to_string_pretty(&self.data).unwrap_or(String::new())
        )
    }
}
impl std::error::Error for PocketBaseError {}

#[allow(dead_code)]
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Claims {
    pub id: String,
    pub collection_id: String,
    pub exp: i64,
    pub refreshable: bool,
    #[serde(rename = "type")]
    pub ty: String,
}

impl Claims {
    pub unsafe fn decode_unsafe(token: &str) -> Result<Claims, Error> {
        let token: jwt::Token<jwt::Header, Claims, _> = jwt::Token::parse_unverified(token)?;
        Ok(token.claims().clone())
    }
}

#[derive(Clone)]
pub struct Token {
    pub auth: String,
    pub expires: DateTime<Utc>,
    pub refreshable: bool,
    pub ty: String,
    pub collection: String,
}

#[derive(Clone)]
pub struct PocketBase {
    client: HttpClient,
    base_uri: String,

    pub token: Option<Token>,
}

impl PocketBase {
    pub fn new(base_uri: impl std::fmt::Display) -> Self {
        Self {
            client: HttpClient::new(),
            base_uri: base_uri.to_string(),
            token: None,
        }
    }

    pub fn is_valid(&self) -> bool {
        let now = Utc::now();
        self.token.as_ref().is_some_and(|t| t.expires > now)
    }

    pub async fn auth_refresh(&mut self) -> Result<(), Error> {
        if let Some(Token {
            auth, collection, ..
        }) = self.token.take()
        {
            let result = self
                .client
                .post(format!(
                    "{}/api/collections/{collection}/auth-refresh",
                    self.base_uri,
                ))
                .header("Authorization", auth)
                .send()
                .await?
                .json::<AuthResult>()
                .await?;

            match result {
                AuthResult::Error { message, .. } => {
                    return Err(Error::Custom(message.unwrap_or("failed to authenticate user".into())));
                }
                AuthResult::Success { token } => {
                    let claims = unsafe { Claims::decode_unsafe(&token)? };
                    self.token.replace(Token {
                        collection,
                        auth: token,
                        refreshable: claims.refreshable,
                        ty: claims.ty,
                        expires: Utc.timestamp_opt(claims.exp, 0).unwrap(),
                    });
                }
            }

            return Ok(());
        }
        Err(Error::Custom(
            "unauthorized client; try running a auth_with_* method first"
                .to_string()
        ))
    }

    pub fn collection<'c, I: std::fmt::Display>(
        &'c mut self,
        identifier: I,
    ) -> CollectionBuilder<'c, I> {
        CollectionBuilder {
            pocketbase: self,
            identifier,
        }
    }

    pub fn files<'c>(&'c self) -> FilesBuilder<'c> {
        FilesBuilder { pocketbase: self }
    }

    pub fn create_batch<'c>(&'c mut self) -> BatchBuilder<'c> {
        BatchBuilder {
            pocketbase: self,
            requests: Default::default(),
        }
    }

    pub async fn health(&mut self) -> Result<Health, Error> {
        Ok(self.client
            .get(format!("{}/api/health", self.base_uri))
            .send()
            .await?
            .json()
            .await?)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    pub code: u16,
    pub message: String,
    pub data: Value
}
impl Health {
    pub fn is_healthy(&self) -> bool {
        self.code == 200
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Paginated<T> {
    pub page: usize,
    pub per_page: usize,
    pub total_items: usize,
    pub total_pages: usize,
    pub items: Vec<T>,
}

impl<T: Clone> Clone for Paginated<T> {
    fn clone(&self) -> Self {
        Paginated {
            page: self.page,
            per_page: self.per_page,
            total_items: self.total_items,
            total_pages: self.total_pages,
            items: self.items.clone(),
        }
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Paginated<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Paginated")
            .field("page", &self.page)
            .field("perPage", &self.per_page)
            .field("totalItems", &self.total_items)
            .field("totalPages", &self.total_pages)
            .field("items", &self.items)
            .finish()
    }
}
