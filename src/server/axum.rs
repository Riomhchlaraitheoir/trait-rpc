use crate::Rpc;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;
use std::sync::Arc;

pub async fn serve<S: Rpc>(State(server): State<Arc<S>>, Json(request): Json<S::Request>) -> impl IntoResponse {
    let response = server.process(request).await;
    Json(response).into_response()
}
