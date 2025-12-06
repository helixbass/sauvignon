use sauvignon::{json_from_response, Schema};
use sqlx::{Pool, Postgres};
use tracing_chrome::ChromeLayerBuilder;
use tracing_subscriber::prelude::*;

#[path = "../tests/shared/mod.rs"]
mod shared;

use shared::{get_db_pool, get_schema, pretty_print_json};

async fn run_request(request: &str, expected: &str, schema: &Schema, db_pool: &Pool<Postgres>) {
    let response = schema.request(request, db_pool).await;
    let json = json_from_response(&response);
    assert_eq!(pretty_print_json(&json), pretty_print_json(expected));
}

#[tokio::main]
async fn main() {
    let (chrome_layer, _guard) = ChromeLayerBuilder::new().build();
    tracing_subscriber::registry().with(chrome_layer).init();

    let db_pool = get_db_pool().await.unwrap();
    let schema = get_schema(&db_pool).await.unwrap();

    run_request(
        r#"
            {
              actors {
                favoriteDesigner {
                  name
                }
              }
            }
        "#,
        r#"
            {
              "data": {
                "actors": [
                  {
                    "favoriteDesigner": {
                      "name": "Proenza Schouler"
                    }
                  },
                  {
                    "favoriteDesigner": {
                      "name": "Ralph Lauren"
                    }
                  }
                ]
              }
            }
        "#,
        &schema,
        &db_pool,
    )
    .await;
}
