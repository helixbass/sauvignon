use indoc::indoc;

use sauvignon::json_from_response;

mod shared;

use shared::{get_db_pool, get_schema, pretty_print_json};

async fn validation_test(request: &str, expected: &str) {
    let db_pool = get_db_pool().await.unwrap();
    let schema = get_schema(&db_pool).await.unwrap();
    let response = schema.request(request, &db_pool).await;
    let json = json_from_response(&response);
    assert_eq!(pretty_print_json(&json), pretty_print_json(expected));
}

#[tokio::test]
async fn test_operation_name_uniqueness() {
    validation_test(
        indoc!(
            r#"
            query Whee {
              actorKatie {
                name
              }
            }

            query Whee {
              actors {
                name
              }
            }
        "#
        ),
        r#"
            {
              "errors": [
                {
                  "message": "Non-unique operation names: `Whee`",
                  "locations": [
                    {
                      "line": 1,
                      "column": 1
                    },
                    {
                      "line": 7,
                      "column": 1
                    }
                  ]
                }
              ]
            }
        "#,
    )
    .await;

    validation_test(
        indoc!(
            r#"
            query Whee {
              actorKatie {
                name
              }
            }

            query Whoa {
              actors {
                name
              }
            }

            query Whee {
              actorKatie {
                name
              }
            }

            query Whoa {
              actors {
                name
              }
            }
        "#
        ),
        r#"
            {
              "errors": [
                {
                  "message": "Non-unique operation names: `Whee`, `Whoa`",
                  "locations": [
                    {
                      "line": 1,
                      "column": 1
                    },
                    {
                      "line": 13,
                      "column": 1
                    },
                    {
                      "line": 7,
                      "column": 1
                    },
                    {
                      "line": 19,
                      "column": 1
                    }
                  ]
                }
              ]
            }
        "#,
    )
    .await;

    validation_test(
        r#"
            query Whee {
              actorKatie {
                name
              }
            }

            query Whoa {
              actors {
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

#[tokio::test]
async fn test_lone_anonymous_operation() {
    validation_test(
        indoc!(
            r#"
            query Named {
              actorKatie {
                name
              }
            }

            {
              actors {
                name
              }
            }
        "#
        ),
        r#"
            {
              "errors": [
                {
                  "message": "Anonymous operation must be only operation",
                  "locations": [
                    {
                      "line": 7,
                      "column": 1
                    }
                  ]
                }
              ]
            }
        "#,
    )
    .await;

    validation_test(
        indoc!(
            r#"
            query {
              actorKatie {
                name
              }
            }

            {
              actors {
                name
              }
            }
        "#
        ),
        r#"
            {
              "errors": [
                {
                  "message": "Anonymous operation must be only operation",
                  "locations": [
                    {
                      "line": 1,
                      "column": 1
                    }
                  ]
                }
              ]
            }
        "#,
    )
    .await;
}
