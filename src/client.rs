use futures_lite::io::AsyncRead;
use std::io::Read;

use http_client_multipart::Multipart;
use isahc::{
    AsyncBody, AsyncReadResponseExt, Body, ReadResponseExt,
    http::{
        Extensions, HeaderMap, HeaderName, HeaderValue, StatusCode, Version, request::Builder,
        uri::PathAndQuery,
    },
};
use serde::{Serialize, de::DeserializeOwned};
use url::Url;

use crate::Error;

#[derive(Clone)]
pub struct HttpClient {
    pub base_url: Url,
    client: isahc::HttpClient,
}

impl HttpClient {
    pub fn new(base_url: impl AsRef<str>) -> Self {
        Self {
            client: isahc::HttpClient::new().unwrap(),
            base_url: Url::parse(base_url.as_ref()).unwrap(),
        }
    }

    pub fn get(&self, uri: impl AsRef<str>) -> RequestBuilder<'_> {
        RequestBuilder {
            client: &self.client,
            request_builder: isahc::Request::get(
                self.base_url.join(uri.as_ref()).unwrap().to_string(),
            ),
        }
    }

    pub fn post(&self, uri: impl AsRef<str>) -> RequestBuilder<'_> {
        RequestBuilder {
            client: &self.client,
            request_builder: isahc::Request::post(
                self.base_url.join(uri.as_ref()).unwrap().to_string(),
            ),
        }
    }

    pub fn patch(&self, uri: impl AsRef<str>) -> RequestBuilder<'_> {
        RequestBuilder {
            client: &self.client,
            request_builder: isahc::Request::patch(
                self.base_url.join(uri.as_ref()).unwrap().to_string(),
            ),
        }
    }

    pub fn delete(&self, uri: impl AsRef<str>) -> RequestBuilder<'_> {
        RequestBuilder {
            client: &self.client,
            request_builder: isahc::Request::delete(
                self.base_url.join(uri.as_ref()).unwrap().to_string(),
            ),
        }
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
