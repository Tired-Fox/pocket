use std::io::Cursor;

use chrono::{TimeZone, Utc};
use http_client_multipart::Multipart;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

use crate::{
    AuthorizedClient, Claims, CreateOptions, Error, ListOptions, Paginated, PocketBaseError, Token, UpdateOptions, ViewOptions, client::{AuthResult, PocketBaseClient}, files::File
};

pub struct CollectionBuilder<'c, P: PocketBaseClient, I: std::fmt::Display> {
    pub(crate) pocketbase: &'c P,
    pub(crate) identifier: I,
}

impl<'c, P: PocketBaseClient, N> CollectionBuilder<'c, P, N>
where
    N: std::fmt::Display,
{
    pub async fn auth_with_password(
        &mut self,
        identifier: &str,
        secret: &str,
    ) -> Result<AuthorizedClient, Error> {
        let result = self
            .pocketbase
            .post(format!(
                "/api/collections/{}/auth-with-password",
                self.identifier,
            ))
            .json(&json!({
                "identity": identifier,
                "password": secret,
            }))?
            .send_async()
            .await?
            .json_async::<AuthResult>()
            .await
            .unwrap();

        match result {
            AuthResult::Error { message, data, .. } => {
                Err(Error::Authorization {
                    message: message
                        .clone()
                        .unwrap_or("failed to authenticate user".into()),
                    data,
                })
            }
            AuthResult::Success { token, record } => {
                let claims = unsafe { Claims::decode_unsafe(&token)? };
                Ok(AuthorizedClient::new(
                    self.pocketbase.base_uri(),
                    Token {
                        user: record.as_object().unwrap().get("id").unwrap().as_str().unwrap().to_string(),
                        collection: self.identifier.to_string(),

                        auth: token.clone(),
                        refreshable: claims.refreshable,
                        ty: claims.ty,
                        expires: Utc.timestamp_opt(claims.exp, 0).unwrap(),
                    }
                ))
            }
        }
    }

    pub async fn get_list<T: DeserializeOwned>(
        self,
        options: ListOptions,
    ) -> Result<Paginated<T>, Error> {
        let res = self
            .pocketbase
            .get(format!("/api/collections/{}/records", self.identifier))
            .query(&options)?
            .send_async()
            .await?;

        if !res.status().is_success() {
            return Err(res.json_async::<PocketBaseError>().await?.into());
        }
        Ok(res.json_async::<Paginated<T>>().await?)
    }

    pub async fn get_one<T: DeserializeOwned>(
        self,
        id: impl std::fmt::Display,
        options: ViewOptions,
    ) -> Result<T, Error> {
        let res = self
            .pocketbase
            .get(format!("/api/collections/{}/records/{id}", self.identifier))
            .query(&options)?
            .send_async()
            .await?;

        if !res.status().is_success() {
            return Err(res.json_async::<PocketBaseError>().await?.into());
        }
        Ok(res.json_async::<T>().await?)
    }

    pub async fn create<R: DeserializeOwned>(
        self,
        record: impl Serialize,
        files: impl IntoIterator<Item = (String, File)>,
        options: CreateOptions,
    ) -> Result<R, Error> {
        let mut form = http_client_multipart::Multipart::new();

        let record = serde_json::to_value(record)?;
        let fields = record.as_object().ok_or(Error::Custom(
            "expected record to be a mapping of fields to values".to_string(),
        ))?;

        for (name, value) in fields {
            let text = match value {
                Value::Null => continue,
                Value::Bool(v) => v.to_string(),
                Value::Number(v) => v.to_string(),
                Value::String(v) => v.to_string(),
                Value::Array(v) => serde_json::to_string(v)?,
                Value::Object(v) => serde_json::to_string(v)?,
            };
            form.add_text(name.to_string(), text);
        }

        for (name, file) in files.into_iter() {
            match file {
                File::Path(path) => form
                    .add_file(name, path, None)
                    .await
                    .map_err(Error::custom)?,
                File::Raw {
                    filename,
                    mime,
                    bytes,
                } => form
                    .add_sync_read(name, filename, &mime, None, Cursor::new(bytes))
                    .map_err(Error::custom)?,
            }
        }

        let res = self
            .pocketbase
            .post(format!("/api/collections/{}/records", self.identifier))
            .query(&options)?
            .multipart(form)?
            .send_async()
            .await?;

        if !res.status().is_success() {
            return Err(res.json_async::<PocketBaseError>().await?.into());
        }
        Ok(res.json_async::<R>().await?)
    }

    pub async fn update<R: DeserializeOwned>(
        self,
        id: impl std::fmt::Display,
        record: impl Serialize,
        files: impl IntoIterator<Item = (String, File)>,
        options: UpdateOptions,
    ) -> Result<R, Error> {
        let mut form = Multipart::new();

        let record = serde_json::to_value(record)?;
        let fields = record.as_object().ok_or(Error::Custom(
            "expected record to be a mapping of fields to values".to_string(),
        ))?;

        for (name, value) in fields {
            let text = match value {
                Value::Null => continue,
                Value::Bool(v) => v.to_string(),
                Value::Number(v) => v.to_string(),
                Value::String(v) => v.to_string(),
                Value::Array(v) => serde_json::to_string(v)?,
                Value::Object(v) => serde_json::to_string(v)?,
            };
            form.add_text(name.to_string(), text);
        }

        for (name, file) in files.into_iter() {
            match file {
                File::Path(path) => form
                    .add_file(name, path, None)
                    .await
                    .map_err(Error::custom)?,
                File::Raw {
                    filename,
                    mime,
                    bytes,
                } => form
                    .add_sync_read(name, filename, &mime, None, Cursor::new(bytes))
                    .map_err(Error::custom)?,
            }
        }

        let res = self
            .pocketbase
            .patch(format!("/api/collections/{}/records/{id}", self.identifier))
            .query(&options)?
            .multipart(form)?
            .send_async()
            .await?;

        if !res.status().is_success() {
            return Err(res.json_async::<PocketBaseError>().await?.into());
        }
        Ok(res.json_async::<R>().await?)
    }

    pub async fn delete(self, id: impl std::fmt::Display) -> Result<(), Error> {
        let res = self
            .pocketbase
            .delete(format!("/api/collections/{}/records/{id}", self.identifier))
            .send_async()
            .await?;

        if !res.status().is_success() {
            return Err(res.json_async::<PocketBaseError>().await?.into());
        }
        Ok(())
    }
}
