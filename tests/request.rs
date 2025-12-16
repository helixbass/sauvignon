use sauvignon::{json_from_response, PostgresDatabase};

mod shared;

use shared::{get_db_pool, get_schema, pretty_print_json};

async fn request_test(request: &str, expected: &str) {
    let db_pool = get_db_pool().await.unwrap();
    let schema = get_schema(&db_pool).await.unwrap();
    let database = PostgresDatabase::new(db_pool, vec![]);
    let response = schema.request(request, &database).await;
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
                  __typename
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
                      "__typename": "Designer",
                      "name": "Proenza Schouler"
                    },
                    "name": "Katie Cassidy"
                  },
                  {
                    "expression": "Dan where did you go I don't like you",
                    "favoriteActorOrDesigner": {
                      "__typename": "Actor",
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
                __typename
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
                  "__typename": "Actor",
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
                  },
                  {
                    "__typename": "Designer",
                    "name": "Oscar de la Renta"
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

#[tokio::test]
async fn test_parse_error() {
    request_test(
        r#"query abc 1"#,
        r#"
            {
              "errors": [
                {
                  "message": "Expected selection set",
                  "locations": [
                    {
                      "line": 1,
                      "column": 11
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
async fn test_lex_error() {
    request_test(
        r#""abc"#,
        r#"
            {
              "errors": [
                {
                  "message": "expected closing double-quote",
                  "locations": [
                    {
                      "line": 1,
                      "column": 4
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
async fn test_skip_include() {
    request_test(
        r#"
            query {
              actorKatie {
                name @skip(if: true)
                expression
              }
            }
        "#,
        r#"
            {
              "data": {
                "actorKatie": {
                  "expression": "no Serena you can't have the key"
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
                name @include(if: false)
                expression
              }
            }
        "#,
        r#"
            {
              "data": {
                "actorKatie": {
                  "expression": "no Serena you can't have the key"
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
                name @skip(if: false)
                expression @include(if: true)
              }
            }
        "#,
        r#"
            {
              "data": {
                "actorKatie": {
                  "name": "Katie Cassidy",
                  "expression": "no Serena you can't have the key"
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
                name @include(if: false) @skip(if: false)
                expression
              }
            }
        "#,
        r#"
            {
              "data": {
                "actorKatie": {
                  "expression": "no Serena you can't have the key"
                }
              }
            }
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_concrete_sub_field() {
    request_test(
        r#"
            query {
              actorKatie {
                name
                favoriteDesigner {
                  name
                }
              }
            }
        "#,
        r#"
            {
              "data": {
                "actorKatie": {
                  "name": "Katie Cassidy",
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

#[tokio::test]
async fn test_introspection_possible_types() {
    request_test(
        r#"
            {
              __type(name: "ActorOrDesigner") {
                name
                possibleTypes {
                  name
                }
              }
            }
        "#,
        r#"
            {
              "data": {
                "__type": {
                  "name": "ActorOrDesigner",
                  "possibleTypes": [
                    {
                      "name": "Actor"
                    },
                    {
                      "name": "Designer"
                    }
                  ]
                }
              }
            }
        "#,
    )
    .await;

    request_test(
        r#"
            {
              __type(name: "HasName") {
                name
                possibleTypes {
                  name
                }
              }
            }
        "#,
        r#"
            {
              "data": {
                "__type": {
                  "name": "HasName",
                  "possibleTypes": [
                    {
                      "name": "Actor"
                    },
                    {
                      "name": "Designer"
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
async fn test_enum() {
    request_test(
        r#"
            query {
              bestCanadianCity
            }
        "#,
        r#"
            {
              "data": {
                "bestCanadianCity": "VANCOUVER"
              }
            }
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_enum_arguments() {
    request_test(
        r#"
            {
              canadianCityQuote(city: VANCOUVER)
            }
        "#,
        r#"
            {
              "data": {
                "canadianCityQuote": "We're the best"
              }
            }
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_has_many() {
    request_test(
        r#"
            {
              designers {
                name
                favoriteOfActors {
                  name
                }
              }
            }
        "#,
        r#"
            {
              "data": {
                "designers": [
                  {
                    "name": "Proenza Schouler",
                    "favoriteOfActors": [
                      {
                        "name": "Katie Cassidy"
                      }
                    ]
                  },
                  {
                    "name": "Ralph Lauren",
                    "favoriteOfActors": [
                      {
                        "name": "Jessica Szohr"
                      }
                    ]
                  },
                  {
                    "name": "Oscar de la Renta",
                    "favoriteOfActors": []
                  }
                ]
              }
            }
        "#,
    )
    .await;
}

#[tokio::test]
async fn test_has_many_through() {
    request_test(
        r#"
            {
              actorKatie {
                favoriteDesigners {
                  name
                }
              }
            }
        "#,
        r#"
            {
              "data": {
                "actorKatie": {
                  "favoriteDesigners": [
                    {
                      "name": "Proenza Schouler"
                    },
                    {
                      "name": "Oscar de la Renta"
                    }
                  ]
                }
              }
            }
        "#,
    )
    .await;
}
