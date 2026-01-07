//! # Browser
//!
//! This provides a transport implementation which uses the Browser's Fetch API to send http
//! requests and parse the response body as JSON

use bon::bon;
use crate::{AsyncTransport};
use thiserror::Error;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_futures::wasm_bindgen::JsCast;
use web_sys::wasm_bindgen::JsValue;
use web_sys::{Request, RequestInit, RequestMode, Response, Window};
use web_sys::js_sys::{Uint8Array};
use crate::client::ResponseError;
use crate::format::FormatInfo;

/// A client which uses the browsers Fetch API along with JSON format (via serde),
/// only supported on wasm32 architecture
#[derive(Debug, Clone)]
pub struct Browser {
    url: String,
    window: Window,
    request_options: RequestInit,
}

#[bon]
impl Browser {
    /// Create a new client
    #[builder]
    pub fn new(
        /// The url that the service can be reached at
        url: &str,
        /// The HTTP method to use, default: POST
        method: Option<&str>,
        /// The request [mode](https://developer.mozilla.org/en-US/docs/Web/API/RequestInit#mode) to be used, defaults to [cors](RequestMode::Cors)
        mode: Option<RequestMode>,
    ) -> Self {
        // apply defaults
        let method = method.unwrap_or("POST");
        let mode = mode.unwrap_or(RequestMode::Cors);

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

impl AsyncTransport for Browser {
    type Error = Error;

    async fn send(&self, request: Vec<u8>, format_info: &FormatInfo) -> Result<Result<Vec<u8>, ResponseError>, Self::Error> {
        let opts = self.request_options.clone();
        let body = Uint8Array::from(request.as_slice());
        opts.set_body(&body);

        let request =
            Request::new_with_str_and_init(&self.url, &opts).map_err(Error::NewRequest)?;
        request
            .headers()
            .set("Content-Type", format_info.http_content_type)
            .map_err(Error::SetHeader)?;

        let promise = self.window.fetch_with_request(&request);
        let future = JsFuture::from(promise);
        let response = future.await.map_err(Error::Fetch)?;
        let response: Response = response.dyn_into().map_err(Error::CastResponse)?;
        let body = response.array_buffer().map_err(Error::ReadBody)?;
        let body = JsFuture::from(body).await.map_err(Error::ReadBody)?;
        let body = Uint8Array::new(&body).to_vec();
        match response.status() {
            200..=299 => Ok(Ok(body)),
            400..=499 => Ok(Err(ResponseError::BadRequest(String::from_utf8(body).unwrap()))),
            500..=599 => Ok(Err(ResponseError::InternalServerError(String::from_utf8(body).unwrap()))),
            _ => Ok(Err(ResponseError::Unexpected))
        }
    }
}

/// This represents the various errors which can occur when using the Fetch API
#[derive(Debug, Error)]
pub enum Error {
    /// An error occurred while build the request
    #[error("Failed to create new request: {0:?}")]
    NewRequest(JsValue),
    /// An error occurred when setting request headers
    #[error("Failed to set request headers: {0:?}")]
    SetHeader(JsValue),
    /// An error occurred when calling `fetch`
    #[error("Failed to send request: {0:?}")]
    Fetch(JsValue),
    /// The response could not be cast to the expected type
    #[error("Response value is unexpected type: {0:?}")]
    CastResponse(JsValue),
    /// Failed to read binary body
    #[error("Failed to read binary body: {0:?}")]
    ReadBody(JsValue),
}
