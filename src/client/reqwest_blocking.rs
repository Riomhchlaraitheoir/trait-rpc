use bon::bon;
use crate::BlockingTransport;
pub use reqwest::Error;
use reqwest::blocking::Client;
use reqwest::{Method};
use crate::format::{FormatInfo};

/// A AsyncClient which uses the [reqwest] crate
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

    fn send(&self, request: Vec<u8>, format_info: &FormatInfo) -> Result<Vec<u8>, Self::Error> {
        self
            .client
            .request(self.method.clone(), &self.url)
            .body(request)
            .header(reqwest::header::CONTENT_TYPE, format_info.http_content_type)
            .send()?
            .json()
    }
}