use sauvignon::json_from_response;
use tracing_chrome::ChromeLayerBuilder;
use tracing_subscriber::prelude::*;

#[path = "../tests/shared/mod.rs"]
mod shared;

use shared::{get_db_pool, get_schema, pretty_print_json};

async fn request_test(request: &str, expected: &str) {
    let db_pool = get_db_pool().await.unwrap();
    let schema = get_schema(&db_pool).await.unwrap();
    let response = schema.request(request, &db_pool).await;
    let json = json_from_response(&response);
    assert_eq!(pretty_print_json(&json), pretty_print_json(expected));
}

#[tokio::main]
async fn main() {
    let (chrome_layer, _guard) = ChromeLayerBuilder::new().build();
    tracing_subscriber::registry().with(chrome_layer).init();

    request_test(
        r#"
            query {
              actorKatie {
                name
              }
            }
        "#,
        r#"
            {
              "data": {
                "actorKatie": {
                  "name": "Katie Cassidy"
                }
              }
            }
        "#,
    )
    .await;

    request_test(
        r#"
            query {
              actorKatie {
                name
              }
            }
        "#,
        r#"
            {
              "data": {
                "actorKatie": {
                  "name": "Katie Cassidy"
                }
              }
            }
        "#,
    )
    .await;
}
