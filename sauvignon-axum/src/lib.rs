use std::sync::Arc;

pub use axum;

use axum::{
    extract::{FromRequest, Request},
    http::StatusCode,
    response::{IntoResponse, Response as AxumResponse},
    Extension, Json,
};
use sauvignon::{Response, Schema};
use serde::Deserialize;
use sqlx::{Pool, Postgres};

#[axum::debug_handler]
pub async fn graphql(
    Extension(schema): Extension<Arc<Schema>>,
    Extension(db_pool): Extension<Pool<Postgres>>,
    GraphQLRequest(request): GraphQLRequest,
) -> GraphQLResponse {
    GraphQLResponse(schema.request(&request.query, &db_pool).await)
}

pub struct GraphQLRequest(RequestFields);

impl<TState> FromRequest<TState> for GraphQLRequest
where
    TState: Send + Sync,
{
    type Rejection = AxumResponse;

    async fn from_request(request: Request, state: &TState) -> Result<Self, Self::Rejection> {
        Json::<RequestFields>::from_request(request, state)
            .await
            .map(|json| Self(json.0))
            .map_err(|err| {
                (StatusCode::BAD_REQUEST, format!("Invalid JSON body: {err}")).into_response()
            })
    }
}

#[derive(Deserialize)]
pub struct RequestFields {
    pub query: String,
}

pub struct GraphQLResponse(Response);

impl IntoResponse for GraphQLResponse {
    // TODO: should return different status codes for
    // errors eg for validation/parsing error vs
    // runtime errors?
    fn into_response(self) -> AxumResponse {
        Json(self.0).into_response()
    }
}
