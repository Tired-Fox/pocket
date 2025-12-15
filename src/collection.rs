use chrono::{TimeZone, Utc};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{Error, files::File, AuthResult, Claims, ExtendAuth, Paginated, PocketBase, PocketBaseError, Token};

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
                    self.pocketbase.base_uri,
                    self.identifier,
            ))
            .json(&json!({
                "identity": identifier,
                "password": secret,
            }))
            .send()
            .await?
            .json::<AuthResult>()
            .await
            .unwrap();

        match result {
            AuthResult::Error { message, .. } => {
                return Err(Error::Custom(message.unwrap_or("failed to authenticate user".into())));
            }
            AuthResult::Success { token } => {
                let claims = unsafe { Claims::decode_unsafe(&token)? };
                self.pocketbase.token.replace(Token {
                    collection: self.identifier.to_string(),

                    auth: token,
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

        let res = self
            .pocketbase
            .client
            .get(uri)
            .auth(self.pocketbase)
            .await?
            .query(&options)
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(res.json::<PocketBaseError>().await?.into());
        }
        Ok(res.json::<Paginated<T>>().await?)
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

        let res = self
            .pocketbase
            .client
            .get(uri)
            .auth(self.pocketbase)
            .await?
            .query(&options)
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(res.json::<PocketBaseError>().await?.into());
        }
        Ok(res.json::<T>().await?)
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

        let mut form = reqwest::multipart::Form::new();

        let record = serde_json::to_value(record)?;
        let fields = record
            .as_object()
            .ok_or(Error::Custom("expected record to be a mapping of fields to values".to_string()))?;

        for (name, value) in fields {
            let text = match value {
                Value::Null => continue,
                Value::Bool(v) => v.to_string(),
                Value::Number(v) => v.to_string(),
                Value::String(v) => v.to_string(),
                Value::Array(v) => serde_json::to_string(v)?,
                Value::Object(v) => serde_json::to_string(v)?,
            };
            form = form.text(name.to_string(), text);
        }

        for (name, file) in files.into_iter() {
            form = form.part(name.to_string(), file.into_form_part().await?);
        }

        let res = self
            .pocketbase
            .client
            .post(uri)
            .auth(self.pocketbase)
            .await?
            .query(&options)
            .multipart(form)
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(res.json::<PocketBaseError>().await?.into());
        }
        Ok(res.json::<R>().await?)
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

        let mut form = reqwest::multipart::Form::new();

        let record = serde_json::to_value(record)?;
        let fields = record
            .as_object()
            .ok_or(Error::Custom("expected record to be a mapping of fields to values".to_string()))?;

        for (name, value) in fields {
            let text = match value {
                Value::Null => continue,
                Value::Bool(v) => v.to_string(),
                Value::Number(v) => v.to_string(),
                Value::String(v) => v.to_string(),
                Value::Array(v) => serde_json::to_string(v)?,
                Value::Object(v) => serde_json::to_string(v)?,
            };
            form = form.text(name.to_string(), text);
        }

        for (name, file) in files.into_iter() {
            form = form.part(name.to_string(), file.into_form_part().await?);
        }

        let res = self
            .pocketbase
            .client
            .patch(uri)
            .auth(self.pocketbase)
            .await?
            .query(&options)
            .multipart(form)
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(res.json::<PocketBaseError>().await?.into());
        }
        Ok(res.json::<R>().await?)
    }

    pub async fn delete(self, id: impl std::fmt::Display) -> Result<(), Error> {
        let uri = format!(
            "{}/api/collections/{}/records/{id}",
            self.pocketbase.base_uri, self.identifier
        );

        let res = self
            .pocketbase
            .client
            .delete(uri)
            .auth(self.pocketbase)
            .await?
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(res.json::<PocketBaseError>().await?.into());
        }
        Ok(())
    }
}
