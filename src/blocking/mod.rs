use chrono::{TimeZone, Utc};

mod collection;
pub use collection::CollectionBuilder;

mod batch;
pub use batch::BatchBuilder;

use crate::{AuthResult, Claims, Error, Health, HttpClient, Token, files::FilesBuilder};

pub(crate) trait ExtendAuth: Sized {
    fn authenticate(&mut self) -> Result<Option<String>, Error>;
}
impl ExtendAuth for PocketBase {
    fn authenticate(&mut self) -> Result<Option<String>, Error> {
        if self.token.is_some() && !self.is_valid() {
            self.auth_refresh()?;
        }

        Ok(self.token.as_ref().map(|Token { auth, .. }| auth.clone()))
    }
}

#[derive(Clone)]
pub struct PocketBase {
    client: HttpClient,
    base_uri: String,

    pub token: Option<Token>,
}

impl PocketBase {
    pub fn new(base_uri: impl AsRef<str>) -> Self {
        Self {
            client: HttpClient::new(base_uri.as_ref()),
            base_uri: base_uri.as_ref().to_string(),
            token: None,
        }
    }

    pub fn is_valid(&self) -> bool {
        let now = Utc::now();
        self.token.as_ref().is_some_and(|t| t.expires > now)
    }

    pub fn auth_refresh(&mut self) -> Result<(), Error> {
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
                .send()?
                .json::<AuthResult>()?;

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
            base_uri: &self.base_uri,
        }
    }

    pub fn create_batch<'c>(&'c mut self) -> BatchBuilder<'c> {
        BatchBuilder {
            pocketbase: self,
            requests: Default::default(),
        }
    }

    pub fn health(&mut self) -> Result<Health, Error> {
        Ok(self
            .client
            .get(format!("{}/api/health", self.base_uri))
            .send()?
            .json()?)
    }
}
