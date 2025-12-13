use sauvignon::{json_from_response, Schema};
use sqlx::{Pool, Postgres};

mod shared;

use shared::{get_db_pool, get_schema, pretty_print_json};

async fn request_test(schema: &Schema, db_pool: &Pool<Postgres>, request: &str, expected: &str) {
    let response = schema.request(request, &db_pool).await;
    let json = json_from_response(&response);
    assert_eq!(pretty_print_json(&json), pretty_print_json(expected));
}

#[tokio::test]
async fn test_column_getter() {
    let db_pool = get_db_pool().await.unwrap();
    let schema = get_schema(&db_pool).await.unwrap();

    request_test(
        &schema,
        &db_pool,
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
              actors {
                name
              }
              actor(id: 2) {
                expression
              }
              certainActorOrDesigner {
                ... on Actor {
                  expression
                }
                ... on Designer {
                  name
                }
              }
              bestHasName {
                __typename
                name
              }
              actorsAndDesigners {
                ... on Actor {
                  __typename
                  expression
                }
                ... on Designer {
                  __typename
                  name
                }
              }
              bestCanadianCity
              canadianCityQuote(city: VANCOUVER)
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
                },
                "actors": [
                  {
                    "name": "Katie Cassidy"
                  },
                  {
                    "name": "Jessica Szohr"
                  }
                ],
                "actor": {
                  "expression": "Dan where did you go I don't like you"
                },
                "certainActorOrDesigner": {
                  "name": "Proenza Schouler"
                },
                "bestHasName": {
                  "__typename": "Actor",
                  "name": "Katie Cassidy"
                },
                "actorsAndDesigners": [
                  {
                    "__typename": "Actor",
                    "expression": "no Serena you can't have the key"
                  },
                  {
                    "__typename": "Actor",
                    "expression": "Dan where did you go I don't like you"
                  },
                  {
                    "__typename": "Designer",
                    "name": "Proenza Schouler"
                  },
                  {
                    "__typename": "Designer",
                    "name": "Ralph Lauren"
                  },
                  {
                    "__typename": "Designer",
                    "name": "Oscar de la Renta"
                  }
                ],
                "bestCanadianCity": "VANCOUVER",
                "canadianCityQuote": "We're the best"
              }
            }
        "#,
    )
    .await;
}
