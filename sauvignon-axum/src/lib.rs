pub use axum;

use axum::Extension;
use sauvignon::Schema;
use sqlx::{Pool, Postgres};

pub async fn graphql(
    Extension(schema): Extension<Schema>,
    Extension(db_pool): Extension<Pool<Postgres>>,
    GraphQLRequest(request): GraphQLRequest,
) -> GraphQLResponse {
    GraphQLResponse(schema.request(request, &db_pool))
}
