use std::io::Cursor;

use chrono::{TimeZone, Utc};
use http_client_multipart::Multipart;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

use crate::{
    AuthResult, Claims, CreateOptions, Error, ListOptions, Paginated, PocketBase, PocketBaseError,
    Token, UpdateOptions, ViewOptions, files::File, non_blocking::ExtendAuth,
};

pub struct CollectionBuilder<'c, I: std::fmt::Display> {
    pub(crate) pocketbase: &'c mut PocketBase,
    pub(crate) identifier: I,
}

impl<'c, N> CollectionBuilder<'c, N>
where
    N: std::fmt::Display,
{
    pub async fn auth_with_password(
        &mut self,
        identifier: &str,
        secret: &str,
    ) -> Result<(), Error> {
        let result = self
            .pocketbase
            .client
            .post(format!(
                "{}/api/collections/{}/auth-with-password",
                self.pocketbase.base_uri, self.identifier,
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

        match &result {
            AuthResult::Error { message, .. } => {
                return Err(Error::Custom(
                    message
                        .clone()
                        .unwrap_or("failed to authenticate user".into()),
                ));
            }
            AuthResult::Success { token } => {
                let claims = unsafe { Claims::decode_unsafe(&token)? };
                self.pocketbase.token.replace(Token {
                    collection: self.identifier.to_string(),

                    auth: token.clone(),
                    refreshable: claims.refreshable,
                    ty: claims.ty,
                    expires: Utc.timestamp_opt(claims.exp, 0).unwrap(),
                });
            }
        }

        Ok(())
    }

    pub async fn get_list<T: DeserializeOwned>(
        self,
        options: ListOptions,
    ) -> Result<Paginated<T>, Error> {
        let uri = format!(
            "{}/api/collections/{}/records",
            self.pocketbase.base_uri, self.identifier
        );

        let token = self.pocketbase.authenticate().await?;
        let res = self
            .pocketbase
            .client
            .get(uri)
            .header(
                "Authorization",
                token.ok_or(Error::custom("client is not authorized"))?,
            )
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
        let uri = format!(
            "{}/api/collections/{}/records/{id}",
            self.pocketbase.base_uri, self.identifier
        );

        let token = self.pocketbase.authenticate().await?;
        let res = self
            .pocketbase
            .client
            .get(uri)
            .header(
                "Authorization",
                token.ok_or(Error::custom("client is not authorized"))?,
            )
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
        let uri = format!(
            "{}/api/collections/{}/records",
            self.pocketbase.base_uri, self.identifier
        );

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

        let token = self.pocketbase.authenticate().await?;
        let res = self
            .pocketbase
            .client
            .post(uri)
            .header(
                "Authorization",
                token.ok_or(Error::custom("client is not authorized"))?,
            )
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
        let uri = format!(
            "{}/api/collections/{}/records/{id}",
            self.pocketbase.base_uri, self.identifier
        );

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

        let token = self.pocketbase.authenticate().await?;
        let res = self
            .pocketbase
            .client
            .patch(uri)
            .header(
                "Authorization",
                token.ok_or(Error::custom("client is not authorized"))?,
            )
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
        let uri = format!(
            "{}/api/collections/{}/records/{id}",
            self.pocketbase.base_uri, self.identifier
        );

        let token = self.pocketbase.authenticate().await?;
        let res = self
            .pocketbase
            .client
            .delete(uri)
            .header(
                "Authorization",
                token.ok_or(Error::custom("client is not authorized"))?,
            )
            .send_async()
            .await?;

        if !res.status().is_success() {
            return Err(res.json_async::<PocketBaseError>().await?.into());
        }
        Ok(())
    }
}
