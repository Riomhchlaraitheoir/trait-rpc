use serde::de::DeserializeOwned;
use serde::Serialize;

pub use reqwest::Error;
use crate::Transport;

pub struct Client {
    client: reqwest::Client,
    url: String,
}

impl Client {
    pub fn new(url: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            url: url.to_string(),
        }
    }
}

impl Transport for Client {
    type Error = Error;

    async fn send<Req, Resp>(&self, request: Req) -> Result<Resp, <Self as Transport>::Error>
    where
        Req: Serialize,
        Resp: DeserializeOwned
    {
        self.client.post(&self.url).json(&request).send().await?.json().await
    }
}