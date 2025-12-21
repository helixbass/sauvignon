use futures::join;
use sauvignon::{json_from_response, Database, PostgresDatabase, Schema};
use tracing_chrome::ChromeLayerBuilder;
use tracing_subscriber::prelude::*;

#[path = "../tests/shared/mod.rs"]
mod shared;

use shared::{get_db_pool, get_schema, pretty_print_json};

async fn run_request(request: &str, expected: &str, schema: &Schema, database: &Database) {
    let response = schema.request(request, database).await;
    let json = json_from_response(&response);
    assert_eq!(pretty_print_json(&json), pretty_print_json(expected));
}

#[tokio::main]
async fn main() {
    let (chrome_layer, _guard) = ChromeLayerBuilder::new().build();
    tracing_subscriber::registry().with(chrome_layer).init();

    let db_pool = get_db_pool().await.unwrap();
    let database = PostgresDatabase::new(db_pool, vec![]);
    let schema = get_schema(&database.pool).await.unwrap();
    let database: Database = database.into();

    let belongs_to = run_request(
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
        &database,
    );

    let list_query = run_request(
        r#"
            {
              actors {
                name
                expression
              }
            }
        "#,
        r#"
            {
              "data": {
                "actors": [
                  {
                    "name": "Katie Cassidy",
                    "expression": "no Serena you can't have the key"
                  },
                  {
                    "name": "Jessica Szohr",
                    "expression": "Dan where did you go I don't like you"
                  }
                ]
              }
            }
        "#,
        &schema,
        &database,
    );

    join!(belongs_to, list_query);
}
