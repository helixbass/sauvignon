use sauvignon::{json_from_response, schema, CarverOrPopulator, Schema};

mod shared;

pub use shared::get_schema;
use shared::{
    get_db_pool, pretty_print_json, ActorsAndDesignersPopulator, ActorsAndDesignersTypePopulator,
};

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
            actor => {
                type => Actor!
                params => [
                    id => Id!
                ]
            }
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
            certainActorOrDesigner => {
                type => ActorOrDesigner!
                internal_dependencies => [
                    type => literal_value("designers")
                    id => literal_value(1)
                ]
            }
            bestHasName => {
                type => HasName!
                internal_dependencies => [
                    type => literal_value("actors")
                    id => literal_value(1)
                ]
            }
            actorsAndDesigners => {
                type => [ActorOrDesigner!]!
                internal_dependencies => [
                    actor_ids => id_column_list(
                        type => Actor
                    )
                    designer_ids => id_column_list(
                        type => Designer
                    )
                ]
                populator => custom {
                    CarverOrPopulator::UnionOrInterfaceTypePopulatorList(
                        Box::new(ActorsAndDesignersTypePopulator::new()),
                        Box::new(ActorsAndDesignersPopulator::new()),
                    )
                }
            }
        ]
        interfaces => [
            HasName => {
                fields => [
                    name => String!
                ]
            }
        ]
        unions => [
            ActorOrDesigner => [Actor, Designer]
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
                  }
                ]
              }
            }
        "#,
    )
    .await;
}
