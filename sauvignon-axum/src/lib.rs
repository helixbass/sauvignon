pub use axum;

use axum::{
    extract::{FromRequest, Request},
    http::StatusCode,
    response::{IntoResponse as _, Response},
    Extension, Json,
};
use sauvignon::Schema;
use serde::Deserialize;
use sqlx::{Pool, Postgres};

pub async fn graphql(
    Extension(schema): Extension<Schema>,
    Extension(db_pool): Extension<Pool<Postgres>>,
    GraphQLRequest(request): GraphQLRequest,
) -> GraphQLResponse {
    GraphQLResponse(schema.request(&request.query, &db_pool))
}

pub struct GraphQLRequest(RequestFields);

impl<TState> FromRequest<TState> for GraphQLRequest
where
    TState: Send + Sync,
{
    type Rejection = Response;

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
