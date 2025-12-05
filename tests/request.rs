use sauvignon::{json_from_response, parse};

mod shared;

use shared::{get_db_pool, get_schema, pretty_print_json};

async fn request_test(request: &str, expected: &str) {
    let db_pool = get_db_pool().await.unwrap();
    let schema = get_schema(&db_pool).await.unwrap();
    let request = parse(request.chars());
    let response = schema.request(request, &db_pool).await;
    let json = json_from_response(&response);
    assert_eq!(pretty_print_json(&json), pretty_print_json(expected));
}

#[tokio::test]
async fn test_object_field() {
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

// TODO: is order guaranteed in the DB results?
// could add eg an explicit order by ID to the ID's getter?
#[tokio::test]
async fn test_list() {
    request_test(
        r#"
            {
              actors {
                name
              }
            }
        "#,
        r#"
            {
              "data": {
                "actors": [
                  {
                    "name": "Katie Cassidy"
                  },
                  {
                    "name": "Jessica Szohr"
                  }
                ]
              }
            }
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_named_fragment() {
    request_test(
        r#"
            {
              actors {
                ...nameFragment
              }
            }

            fragment nameFragment on Actor {
              name
            }
        "#,
        r#"
            {
              "data": {
                "actors": [
                  {
                    "name": "Katie Cassidy"
                  },
                  {
                    "name": "Jessica Szohr"
                  }
                ]
              }
            }
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_inline_fragment() {
    request_test(
        r#"
            {
              actors {
                ... {
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
                    "name": "Katie Cassidy"
                  },
                  {
                    "name": "Jessica Szohr"
                  }
                ]
              }
            }
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_union() {
    request_test(
        r#"
            {
              certainActorOrDesigner {
                ... on Actor {
                  expression
                }
                ... on Designer {
                  name
                }
              }
            }
        "#,
        r#"
            {
              "data": {
                "certainActorOrDesigner": {
                  "name": "Proenza Schouler"
                }
              }
            }
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_union_field() {
    request_test(
        r#"
            {
              actors {
                name
                expression
                favoriteActorOrDesigner {
                  ... on Actor {
                    expression
                  }
                  ... on Designer {
                    name
                  }
                }
              }
            }
        "#,
        r#"
            {
              "data": {
                "actors": [
                  {
                    "expression": "no Serena you can't have the key",
                    "favoriteActorOrDesigner": {
                      "name": "Proenza Schouler"
                    },
                    "name": "Katie Cassidy"
                  },
                  {
                    "expression": "Dan where did you go I don't like you",
                    "favoriteActorOrDesigner": {
                      "expression": "no Serena you can't have the key"
                    },
                    "name": "Jessica Szohr"
                  }
                ]
              }
            }
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_interface() {
    request_test(
        r#"
            {
              actors {
                favoriteActorOrDesigner {
                  ... on HasName {
                    name
                  }
                  ... on Actor {
                    expression
                  }
                }
              }
              bestHasName {
                name
              }
            }
        "#,
        r#"
            {
              "data": {
                "actors": [
                  {
                    "favoriteActorOrDesigner": {
                      "name": "Proenza Schouler"
                    }
                  },
                  {
                    "favoriteActorOrDesigner": {
                      "expression": "no Serena you can't have the key",
                      "name": "Katie Cassidy"
                    }
                  }
                ],
                "bestHasName": {
                  "name": "Katie Cassidy"
                }
              }
            }
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_list_union_and_typename() {
    request_test(
        r#"
            {
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
            }
        "#,
        r#"
            {
              "data": {
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
                  }
                ]
              }
            }
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_introspection_type_interfaces() {
    request_test(
        r#"
            {
              __type(name: "Actor") {
                name
                interfaces {
                  name
                }
              }
            }
        "#,
        r#"
            {
              "data": {
                "__type": {
                  "name": "Actor",
                  "interfaces": [
                    {
                      "name": "HasName"
                    }
                  ]
                }
              }
            }
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_argument() {
    request_test(
        r#"
            {
              actor(id: 1) {
                name
              }
            }
        "#,
        r#"
            {
              "data": {
                "actor": {
                  "name": "Katie Cassidy"
                }
              }
            }
        "#,
    )
    .await;
}
