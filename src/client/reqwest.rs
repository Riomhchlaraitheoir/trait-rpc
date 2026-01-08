//! This module defines the reqwest transport support

use bon::bon;
use crate::AsyncTransport;
pub use reqwest::Error;
use crate::client::ResponseError;

/// An [`AsyncTransport`] which uses the [reqwest] crate
#[derive(Debug, Clone)]
pub struct Reqwest {
    client: reqwest::Client,
    url: String,
    method: reqwest::Method,
}

#[bon]
impl Reqwest {
    /// Create a new client using the given URL and method
    #[builder]
    pub fn new(
        /// The underlying reqwest client
        client: Option<reqwest::Client>,
        /// The url to access the service at
        #[builder(into)]
        url: String,
        /// The HTTP method to use, default is POST
        method: Option<reqwest::Method>
    ) -> Self {
        Self {
            client: client.unwrap_or_default(),
            url,
            method: method.unwrap_or(reqwest::Method::POST),
        }
    }
}

impl AsyncTransport for Reqwest {
    type Error = Error;

    async fn send(&self, request: Vec<u8>, content_type: &str) -> Result<Result<Vec<u8>, ResponseError>, Self::Error> {
        let response = self
            .client
            .request(self.method.clone(), &self.url)
            .body(request)
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .send()
            .await?;
        if response.status().is_success() {
            Ok(Ok(response.json().await?))
        } else if response.status().is_client_error() {
            Ok(Err(ResponseError::BadRequest(response.text().await?)))
        } else if response.status().is_server_error() {
            Ok(Err(ResponseError::InternalServerError(response.text().await?)))
        } else {
            Ok(Err(ResponseError::Unexpected))
        }
    }
}