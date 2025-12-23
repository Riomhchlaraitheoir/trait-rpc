//! # Browser
//!
//! This provides a transport implementation which uses the Browser's Fetch API to send http
//! requests and parse the response body as JSON

use serde::de::DeserializeOwned;
use serde::Serialize;
use thiserror::Error;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_futures::wasm_bindgen::JsCast;
use web_sys::{Request, RequestInit, RequestMode, Response, Window};
use web_sys::wasm_bindgen::JsValue;
use crate::Transport;

pub struct Client {
    url: String,
    window: Window,
    request_options: RequestInit,
}

impl Client {
    pub fn new(url: &str, method: &str, mode: RequestMode) -> Self {
        let window = web_sys::window().expect("no global `window` exists");
        let opts = RequestInit::new();
        opts.set_method(method);
        opts.set_mode(mode);
        Self {
            window,
            url: url.to_string(),
            request_options: opts,
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
        let body = serde_json::to_string(&request).map_err(Error::Serialise)?;
        let opts = self.request_options.clone();
        opts.set_body(&JsValue::from_str(&body));

        let request = Request::new_with_str_and_init(&self.url, &opts).map_err(Error::NewRequest)?;
        request.headers().set("Content-Type", "application/json").map_err(Error::SetHeader)?;

        let promise = self.window.fetch_with_request(&request);
        let future = JsFuture::from(promise);
        let response = future.await.map_err(Error::Fetch)?;
        let response: Response = response.dyn_into().map_err(Error::CastResponse)?;
        let body = response.json().map_err(Error::ParseJson)?;
        let body = JsFuture::from(body).await.map_err(Error::ParseJson)?;
        let response = serde_wasm_bindgen::from_value(body)?;
        Ok(response)
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed to serialise request body: {0}")]
    Serialise(serde_json::Error),
    #[error("Failed to create new request: {0:?}")]
    NewRequest(JsValue),
    #[error("Failed to set request headers: {0:?}")]
    SetHeader(JsValue),
    #[error("Failed to send request: {0:?}")]
    Fetch(JsValue),
    #[error("Response value is unexpected type: {0:?}")]
    CastResponse(JsValue),
    #[error("Failed to parse Json body to javascript object: {0:?}")]
    ParseJson(JsValue),
    #[error("Deserialization from javascript object failed: {0}")]
    Deserialize(#[from] serde_wasm_bindgen::Error),
}
