use sauvignon::{json_from_response, schema, Schema};

mod shared;

pub use shared::get_schema;
use shared::{get_db_pool, pretty_print_json};

async fn request_test(schema: &Schema, request: &str, expected: &str) {
    let db_pool = get_db_pool().await.unwrap();
    let response = schema.request(request, &db_pool).await;
    let json = json_from_response(&response);
    assert_eq!(pretty_print_json(&json), pretty_print_json(expected));
}

#[tokio::test]
async fn test_column_getter() {
    let schema = schema! {
        types => [
            Actor => {
                fields => [
                    name => string_column(),
                ]
            }
        ]
        query => [
            actorKatie => {
                type => Actor!
                internal_dependencies => [
                    id => literal_value(1),
                ]
            }
        ]
    };

    request_test(
        &schema,
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
