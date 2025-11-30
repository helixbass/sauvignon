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

#[tokio::test]
async fn test_type_names_exist() {
    validation_test(
        indoc!(
            r#"
            query {
              actorKatie {
                ... on NonExistent {
                  name
                }
              }
            }

            fragment greatFragment on SomethingNonExistent {
              expression
            }
        "#
        ),
        r#"
            {
              "errors": [
                {
                  "message": "Unknown type name: `NonExistent`",
                  "locations": [
                    {
                      "line": 3,
                      "column": 5
                    }
                  ]
                },
                {
                  "message": "Unknown type name: `SomethingNonExistent`",
                  "locations": [
                    {
                      "line": 9,
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

#[tokio::test]
async fn test_selection_fields_exist() {
    validation_test(
        indoc!(
            r#"
            {
              actorKatie {
                namez
                name {
                  woops
                }
              }
              actors
            }
        "#
        ),
        r#"
            {
              "errors": [
                {
                  "message": "Field `namez` doesn't exist on `Actor`",
                  "locations": [
                    {
                      "line": 3,
                      "column": 5
                    }
                  ]
                },
                {
                  "message": "Field `name` can't have selection set because it is of scalar type `String`",
                  "locations": [
                    {
                      "line": 4,
                      "column": 5
                    }
                  ]
                },
                {
                  "message": "Field `actors` must have selection set because it is of non-scalar type `Actor`",
                  "locations": [
                    {
                      "line": 8,
                      "column": 3
                    }
                  ]
                }
              ]
            }
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_fragment_name_duplicate() {
    validation_test(
        indoc!(
            r#"
            {
              actorKatie {
                ...wheeFragment
              }
            }

            fragment wheeFragment on Actor {
              name
            }

            fragment wheeFragment on Actor {
              expression
            }
        "#
        ),
        r#"
            {
              "errors": [
                {
                  "message": "Non-unique fragment names: `wheeFragment`",
                  "locations": [
                    {
                      "line": 7,
                      "column": 1
                    },
                    {
                      "line": 11,
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
            {
              actorKatie {
                ...wheeFragment
              }
            }

            fragment wheeFragment on Actor {
              name
            }

            fragment wheeFragment on Actor {
              expression
            }

            fragment whoaFragment on Actor {
              name
            }

            fragment wheeFragment on Actor {
              favoriteActorOrDesigner {
                __typename
              }
            }

            fragment whoaFragment on Actor {
              expression
            }
        "#
        ),
        r#"
            {
              "errors": [
                {
                  "message": "Non-unique fragment names: `wheeFragment`, `whoaFragment`",
                  "locations": [
                    {
                      "line": 7,
                      "column": 1
                    },
                    {
                      "line": 11,
                      "column": 1
                    },
                    {
                      "line": 19,
                      "column": 1
                    },
                    {
                      "line": 15,
                      "column": 1
                    },
                    {
                      "line": 25,
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

#[tokio::test]
async fn test_fragment_on_scalar_type() {
    validation_test(
        indoc!(
            r#"
            {
              actorKatie {
                ...wheeFragment
              }
            }

            fragment wheeFragment on String {
              name
            }
        "#
        ),
        r#"
            {
              "errors": [
                {
                  "message": "Fragment `wheeFragment` can't be of scalar type `String`",
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
            {
              actorKatie {
                ... on String {
                  whee
                }
              }
            }
        "#
        ),
        r#"
            {
              "errors": [
                {
                  "message": "Inline fragment can't be of scalar type `String`",
                  "locations": [
                    {
                      "line": 3,
                      "column": 5
                    }
                  ]
                }
              ]
            }
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_unused_fragment() {
    validation_test(
        indoc!(
            r#"
            {
              actorKatie {
                name
              }
            }

            fragment wheeFragment on Actor {
              expression
            }

            fragment whoaFragment on Actor {
                name
            }
        "#
        ),
        r#"
            {
              "errors": [
                {
                  "message": "Unused fragment: `wheeFragment`",
                  "locations": [
                    {
                      "line": 7,
                      "column": 1
                    }
                  ]
                },
                {
                  "message": "Unused fragment: `whoaFragment`",
                  "locations": [
                    {
                      "line": 11,
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
