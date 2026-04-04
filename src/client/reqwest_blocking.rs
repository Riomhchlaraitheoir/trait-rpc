use bon::bon;
use crate::BlockingTransport;
pub use reqwest::Error;
use reqwest::blocking::Client;
use reqwest::{Method};
use crate::client::HandleError;

/// A [`AsyncTransport`] which uses the [reqwest] crate
#[derive(Debug, Clone)]
pub struct ReqwestBlocking {
    client: Client,
    url: String,
    method: reqwest::Method,
}

#[bon]
impl ReqwestBlocking {
    /// Create a new client using the given URL and method
    #[builder]
    pub fn new(
        /// The underlying reqwest client
        client: Option<Client>,
        /// The url to access the service at
        #[builder(into)]
        url: String,
        /// The HTTP method to use, default is POST
        method: Option<Method>) -> Self {
        Self {
            client: client.unwrap_or_default(),
            url,
            method: method.unwrap_or(Method::POST),
        }
    }
}

impl BlockingTransport for ReqwestBlocking {
    type Error = Error;

    fn send(&self, request: Vec<u8>, content_type: &str) -> Result<Result<Vec<u8>, HandleError>, Self::Error> {
        let response = self
            .client
            .request(self.method.clone(), &self.url)
            .body(request)
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .send()?;
        if response.status().is_success() {
            Ok(Ok(response.bytes()?.to_vec()))
        } else if response.status().is_client_error() {
            Ok(Err(HandleError::BadRequest(response.text()?)))
        } else if response.status().is_server_error() {
            Ok(Err(HandleError::InternalServerError(response.text()?)))
        } else {
            Ok(Err(HandleError::Unexpected))
        }
    }
}