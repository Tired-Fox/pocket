use std::{collections::BTreeMap, path::PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub type Record = serde_json::Map<String, Value>;

pub mod blocking;
pub mod non_blocking;
pub use non_blocking::{PocketBase, batch, collection};

mod error;
pub use error::Error;

pub mod files;
pub use files::FilesBuilder;

mod client;
pub(crate) use client::HttpClient;

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
struct PocketBaseError {
    status: u16,
    message: String,
    data: Value,
}
impl std::fmt::Display for PocketBaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
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
        let token = jsonwebtoken::dangerous::insecure_decode::<Claims>(token)?;
        Ok(token.claims)
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    pub code: u16,
    pub message: String,
    pub data: Value,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_page: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expand: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_total: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expand: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expand: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expand: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<String>,
}

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