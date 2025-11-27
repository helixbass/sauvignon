use sqlx::{postgres::PgPoolOptions, Pool, Postgres};

use sauvignon::{
    json_from_response, parse, ArgumentInternalDependencyResolver, CarverOrPopulator, ColumnGetter,
    ColumnGetterList, DependencyType, DependencyValue, ExternalDependency,
    ExternalDependencyValues, FieldResolver, Id, InterfaceBuilder, InterfaceField,
    InternalDependency, InternalDependencyResolver, InternalDependencyValues,
    LiteralValueInternalDependencyResolver, ObjectTypeBuilder, OperationType, Param, PopulatorList,
    Schema, StringCarver, Type, TypeDepluralizer, TypeFieldBuilder, TypeFull, Union,
    UnionOrInterfaceTypePopulatorList, ValuePopulator, ValuePopulatorList, ValuesPopulator,
};

pub struct ActorsAndDesignersTypePopulator {}

impl ActorsAndDesignersTypePopulator {
    pub fn new() -> Self {
        Self {}
    }
}

impl UnionOrInterfaceTypePopulatorList for ActorsAndDesignersTypePopulator {
    fn populate(
        &self,
        _external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> Vec<String> {
        internal_dependencies
            .get("actor_ids")
            .unwrap()
            .as_list()
            .into_iter()
            .map(|_| "Actor".to_owned())
            .chain(
                internal_dependencies
                    .get("designer_ids")
                    .unwrap()
                    .as_list()
                    .into_iter()
                    .map(|_| "Designer".to_owned()),
            )
            .collect()
    }
}

pub struct ActorsAndDesignersPopulator {}

impl ActorsAndDesignersPopulator {
    pub fn new() -> Self {
        Self {}
    }
}

impl PopulatorList for ActorsAndDesignersPopulator {
    fn populate(
        &self,
        _external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> Vec<ExternalDependencyValues> {
        internal_dependencies
            .get("actor_ids")
            .unwrap()
            .as_list()
            .into_iter()
            .map(|actor_id| {
                let mut ret = ExternalDependencyValues::default();
                ret.insert("id".to_owned(), actor_id.clone()).unwrap();
                ret
            })
            .chain(
                internal_dependencies
                    .get("designer_ids")
                    .unwrap()
                    .as_list()
                    .into_iter()
                    .map(|designer_id| {
                        let mut ret = ExternalDependencyValues::default();
                        ret.insert("id".to_owned(), designer_id.clone()).unwrap();
                        ret
                    }),
            )
            .collect()
    }
}

async fn get_schema(db_pool: &Pool<Postgres>) -> anyhow::Result<Schema> {
    let has_name_interface = InterfaceBuilder::default()
        .name("HasName")
        .fields(vec![InterfaceField::new(
            "name".to_owned(),
            TypeFull::Type("String".to_owned()),
        )])
        .build()
        .unwrap();

    let actor_type = Type::Object(
        ObjectTypeBuilder::default()
            .name("Actor")
            .fields([
                TypeFieldBuilder::default()
                    .name("name")
                    .type_(TypeFull::Type("String".to_owned()))
                    // {
                    //   external_dependencies => ["id" => ID],
                    //   internal_dependencies => [
                    //     column_fetcher!(),
                    //   ],
                    //   value => column_value!(),
                    // }
                    // AKA column!()
                    .resolver(FieldResolver::new(
                        vec![ExternalDependency::new("id".to_owned(), DependencyType::Id)],
                        vec![InternalDependency::new(
                            "name".to_owned(),
                            DependencyType::String,
                            InternalDependencyResolver::ColumnGetter(ColumnGetter::new(
                                "actors".to_owned(),
                                "name".to_owned(),
                            )),
                        )],
                        CarverOrPopulator::Carver(Box::new(StringCarver::new("name".to_owned()))),
                    ))
                    .build()
                    .unwrap(),
                TypeFieldBuilder::default()
                    .name("expression")
                    .type_(TypeFull::Type("String".to_owned()))
                    .resolver(FieldResolver::new(
                        vec![ExternalDependency::new("id".to_owned(), DependencyType::Id)],
                        vec![InternalDependency::new(
                            "expression".to_owned(),
                            DependencyType::String,
                            InternalDependencyResolver::ColumnGetter(ColumnGetter::new(
                                "actors".to_owned(),
                                "expression".to_owned(),
                            )),
                        )],
                        CarverOrPopulator::Carver(Box::new(StringCarver::new(
                            "expression".to_owned(),
                        ))),
                    ))
                    .build()
                    .unwrap(),
                TypeFieldBuilder::default()
                    .name("favoriteActorOrDesigner")
                    .type_(TypeFull::Type("ActorOrDesigner".to_owned()))
                    .resolver(FieldResolver::new(
                        vec![ExternalDependency::new("id".to_owned(), DependencyType::Id)],
                        vec![
                            InternalDependency::new(
                                "type".to_owned(),
                                DependencyType::String,
                                InternalDependencyResolver::ColumnGetter(ColumnGetter::new(
                                    "actors".to_owned(),
                                    "favorite_actor_or_designer_type".to_owned(),
                                )),
                            ),
                            InternalDependency::new(
                                "favorite_actor_or_designer_id".to_owned(),
                                DependencyType::Id,
                                InternalDependencyResolver::ColumnGetter(ColumnGetter::new(
                                    "actors".to_owned(),
                                    "favorite_actor_or_designer_id".to_owned(),
                                )),
                            ),
                        ],
                        CarverOrPopulator::UnionOrInterfaceTypePopulator(
                            Box::new(TypeDepluralizer::new()),
                            Box::new(ValuesPopulator::new([(
                                "favorite_actor_or_designer_id".to_owned(),
                                "id".to_owned(),
                            )])),
                        ),
                    ))
                    .build()
                    .unwrap(),
            ])
            .implements(vec!["HasName".to_owned()])
            .build()
            .unwrap(),
    );

    let designer_type = Type::Object(
        ObjectTypeBuilder::default()
            .name("Designer")
            .fields([TypeFieldBuilder::default()
                .name("name")
                .type_(TypeFull::Type("String".to_owned()))
                .resolver(FieldResolver::new(
                    vec![ExternalDependency::new("id".to_owned(), DependencyType::Id)],
                    vec![InternalDependency::new(
                        "name".to_owned(),
                        DependencyType::String,
                        InternalDependencyResolver::ColumnGetter(ColumnGetter::new(
                            "designers".to_owned(),
                            "name".to_owned(),
                        )),
                    )],
                    CarverOrPopulator::Carver(Box::new(StringCarver::new("name".to_owned()))),
                ))
                .build()
                .unwrap()])
            .implements(vec!["HasName".to_owned()])
            .build()
            .unwrap(),
    );

    let actor_or_designer = Union::new(
        "ActorOrDesigner".to_owned(),
        vec!["Actor".to_owned(), "Designer".to_owned()],
    );

    let (katie_id,): (Id,) = sqlx::query_as("SELECT id FROM actors WHERE name = 'Katie Cassidy'")
        .fetch_one(db_pool)
        .await
        .unwrap();
    let (proenza_schouler_id,): (Id,) =
        sqlx::query_as("SELECT id FROM designers WHERE name = 'Proenza Schouler'")
            .fetch_one(db_pool)
            .await
            .unwrap();

    let query_type = Type::Object(
        ObjectTypeBuilder::default()
            .name("Query")
            .fields([
                TypeFieldBuilder::default()
                    .name("actor")
                    .type_(TypeFull::Type("Actor".to_owned()))
                    // {
                    //   external_dependencies => None,
                    //   internal_dependencies => {"id" => Argument},
                    //   populator => ValuePopulator("id"),
                    // }
                    .resolver(FieldResolver::new(
                        vec![],
                        vec![InternalDependency::new(
                            "id".to_owned(),
                            DependencyType::Id,
                            InternalDependencyResolver::Argument(
                                ArgumentInternalDependencyResolver::new("id".to_owned()),
                            ),
                        )],
                        CarverOrPopulator::Populator(Box::new(ValuePopulator::new(
                            "id".to_owned(),
                        ))),
                    ))
                    .params([Param::new(
                        "id".to_owned(),
                        // TODO: presumably non-null?
                        TypeFull::Type("Id".to_owned()),
                    )])
                    .build()
                    .unwrap(),
                TypeFieldBuilder::default()
                    .name("actors")
                    .type_(TypeFull::List("Actor".to_owned()))
                    // {
                    //   external_dependencies => None,
                    //   internal_dependencies => [
                    //      column_fetcher_list!("actors", "id"),
                    //   ],
                    //   populator =>
                    // }
                    .resolver(FieldResolver::new(
                        vec![],
                        vec![InternalDependency::new(
                            "ids".to_owned(),
                            DependencyType::ListOfIds,
                            InternalDependencyResolver::ColumnGetterList(ColumnGetterList::new(
                                "actors".to_owned(),
                                "id".to_owned(),
                            )),
                        )],
                        CarverOrPopulator::PopulatorList(Box::new(ValuePopulatorList::new(
                            "id".to_owned(),
                        ))),
                    ))
                    .build()
                    .unwrap(),
                TypeFieldBuilder::default()
                    .name("actorKatie")
                    .type_(TypeFull::Type("Actor".to_owned()))
                    // {
                    //   external_dependencies => None,
                    //   internal_dependencies => {"id" => LiteralValue(4)},
                    //   populator => ValuePopulator("id"),
                    // }
                    .resolver(FieldResolver::new(
                        vec![],
                        vec![InternalDependency::new(
                            "id".to_owned(),
                            DependencyType::Id,
                            InternalDependencyResolver::LiteralValue(
                                LiteralValueInternalDependencyResolver(DependencyValue::Id(
                                    katie_id,
                                )),
                            ),
                        )],
                        CarverOrPopulator::Populator(Box::new(ValuePopulator::new(
                            "id".to_owned(),
                        ))),
                    ))
                    .build()
                    .unwrap(),
                TypeFieldBuilder::default()
                    .name("certainActorOrDesigner")
                    .type_(TypeFull::Type("ActorOrDesigner".to_owned()))
                    .resolver(FieldResolver::new(
                        vec![],
                        vec![
                            InternalDependency::new(
                                "type".to_owned(),
                                DependencyType::String,
                                InternalDependencyResolver::LiteralValue(
                                    LiteralValueInternalDependencyResolver(
                                        DependencyValue::String("designers".to_owned()),
                                    ),
                                ),
                            ),
                            InternalDependency::new(
                                "id".to_owned(),
                                DependencyType::Id,
                                InternalDependencyResolver::LiteralValue(
                                    LiteralValueInternalDependencyResolver(DependencyValue::Id(
                                        proenza_schouler_id,
                                    )),
                                ),
                            ),
                        ],
                        CarverOrPopulator::UnionOrInterfaceTypePopulator(
                            Box::new(TypeDepluralizer::new()),
                            Box::new(ValuePopulator::new("id".to_owned())),
                        ),
                    ))
                    .build()
                    .unwrap(),
                TypeFieldBuilder::default()
                    .name("bestHasName")
                    .type_(TypeFull::Type("HasName".to_owned()))
                    .resolver(FieldResolver::new(
                        vec![],
                        vec![
                            InternalDependency::new(
                                "type".to_owned(),
                                DependencyType::String,
                                InternalDependencyResolver::LiteralValue(
                                    LiteralValueInternalDependencyResolver(
                                        DependencyValue::String("actors".to_owned()),
                                    ),
                                ),
                            ),
                            InternalDependency::new(
                                "id".to_owned(),
                                DependencyType::Id,
                                InternalDependencyResolver::LiteralValue(
                                    LiteralValueInternalDependencyResolver(DependencyValue::Id(
                                        katie_id,
                                    )),
                                ),
                            ),
                        ],
                        CarverOrPopulator::UnionOrInterfaceTypePopulator(
                            Box::new(TypeDepluralizer::new()),
                            Box::new(ValuePopulator::new("id".to_owned())),
                        ),
                    ))
                    .build()
                    .unwrap(),
                TypeFieldBuilder::default()
                    .name("actorsAndDesigners")
                    .type_(TypeFull::List("ActorOrDesigner".to_owned()))
                    .resolver(FieldResolver::new(
                        vec![],
                        vec![
                            InternalDependency::new(
                                "actor_ids".to_owned(),
                                DependencyType::ListOfIds,
                                InternalDependencyResolver::ColumnGetterList(
                                    ColumnGetterList::new("actors".to_owned(), "id".to_owned()),
                                ),
                            ),
                            InternalDependency::new(
                                "designer_ids".to_owned(),
                                DependencyType::ListOfIds,
                                InternalDependencyResolver::ColumnGetterList(
                                    ColumnGetterList::new("designers".to_owned(), "id".to_owned()),
                                ),
                            ),
                        ],
                        CarverOrPopulator::UnionOrInterfaceTypePopulatorList(
                            Box::new(ActorsAndDesignersTypePopulator::new()),
                            Box::new(ActorsAndDesignersPopulator::new()),
                        ),
                    ))
                    .build()
                    .unwrap(),
            ])
            .is_top_level_type(OperationType::Query)
            .build()
            .unwrap(),
    );

    Ok(Schema::try_new(
        vec![query_type, actor_type, designer_type],
        vec![actor_or_designer],
        vec![has_name_interface],
    )?)
}

async fn get_db_pool() -> anyhow::Result<Pool<Postgres>> {
    let db_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://sauvignon:password@localhost/sauvignon")
        .await?;

    Ok(db_pool)
}

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

fn pretty_print_json(json: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
    serde_json::to_string_pretty(&parsed).unwrap()
}
