pub mod batch;
use batch::BatchBuilder;

pub mod collection;
use chrono::{TimeZone, Utc};
use collection::CollectionBuilder;

use crate::{AuthResult, Claims, Error, FilesBuilder, Health, HttpClient, Token};

pub(crate) trait ExtendAuth: Sized {
    fn authenticate(&mut self) -> impl Future<Output = Result<Option<String>, Error>> + Send;
}
impl ExtendAuth for PocketBase {
    async fn authenticate(&mut self) -> Result<Option<String>, Error> {
        if self.token.is_some() && !self.is_valid() {
            self.auth_refresh().await?;
        }

        Ok(self.token.as_ref().map(|Token { auth, .. }| auth.clone()))
    }
}

#[derive(Clone)]
pub struct PocketBase {
    client: HttpClient,
    pub token: Option<Token>,
}

impl PocketBase {
    pub fn new(base_uri: impl AsRef<str>) -> Self {
        Self {
            client: HttpClient::new(base_uri.as_ref()),
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
                .post("/api/collections/{collection}/auth-refresh")
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
            "unauthorized client; try running a auth_with_* method first".to_string(),
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
        FilesBuilder {
            base_url: &self.client.base_url,
        }
    }

    pub fn create_batch<'c>(&'c mut self) -> BatchBuilder<'c> {
        BatchBuilder {
            pocketbase: self,
            requests: Default::default(),
        }
    }

    pub async fn health(&mut self) -> Result<Health, Error> {
        Ok(self
            .client
            .get("/api/health")
            .send_async()
            .await?
            .json_async()
            .await?)
    }
}
