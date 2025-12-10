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
                    name => string_column()
                    expression => string_column(),
                    favoriteDesigner => belongs_to(
                        type => Designer
                    )
                ]
                implements => [HasName]
            }
            Designer => {
                fields => [
                    name => string_column()
                ]
                implements => [HasName]
            }
        ]
        query => [
            actorKatie => {
                type => Actor!
                internal_dependencies => [
                    id => literal_value(1),
                ]
            }
            actors => {
                type => [Actor!]!
                internal_dependencies => [
                    ids => id_column_list()
                ]
            }
        ]
        interfaces => [
            HasName => {
                fields => [
                    name => String!
                ]
            }
        ]
    };

    request_test(
        &schema,
        r#"
            query {
              actorKatie {
                ... on HasName {
                  name
                }
                expression
                favoriteDesigner {
                  ... on HasName {
                    name
                  }
                }
              }
            }
        "#,
        r#"
            {
              "data": {
                "actorKatie": {
                  "name": "Katie Cassidy",
                  "expression": "no Serena you can't have the key",
                  "favoriteDesigner": {
                    "name": "Proenza Schouler"
                  }
                }
              }
            }
        "#,
    )
    .await;
}
