use sqlx::postgres::PgPoolOptions;

use sauvignon::{
    json_from_response, ArgumentInternalDependencyResolver, CarverOrPopulator, ColumnGetter,
    ColumnGetterList, DependencyType, DependencyValue, Document, ExecutableDefinition,
    ExternalDependency, ExternalDependencyValues, FieldResolver, FragmentDefinition,
    FragmentSpread, Id, IdPopulatorList, InlineFragment, Interface, InterfaceField,
    InternalDependency, InternalDependencyResolver, InternalDependencyValues,
    LiteralValueInternalDependencyResolver, ObjectType, OperationDefinition, OperationType,
    PopulatorList, Request, Schema, Selection, SelectionField, SelectionSet, StringColumnCarver,
    Type, TypeDepluralizer, TypeField, TypeFull, Union, UnionOrInterfaceTypePopulatorList,
    ValuePopulator, ValuesPopulator,
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://sauvignon:password@localhost/sauvignon")
        .await?;

    let has_name_interface = Interface::new(
        "HasName".to_owned(),
        vec![InterfaceField::new(
            "name".to_owned(),
            TypeFull::Type("String".to_owned()),
        )],
        vec![],
    );

    let actor_type = Type::Object(ObjectType::new(
        "Actor".to_owned(),
        vec![
            TypeField::new(
                "name".to_owned(),
                TypeFull::Type("String".to_owned()),
                // {
                //   external_dependencies => ["id" => ID],
                //   internal_dependencies => [
                //     column_fetcher!(),
                //   ],
                //   value => column_value!(),
                // }
                // AKA column!()
                FieldResolver::new(
                    vec![ExternalDependency::new("id".to_owned(), DependencyType::Id)],
                    vec![InternalDependency::new(
                        "name".to_owned(),
                        DependencyType::String,
                        InternalDependencyResolver::ColumnGetter(ColumnGetter::new(
                            "actors".to_owned(),
                            "name".to_owned(),
                        )),
                    )],
                    CarverOrPopulator::Carver(Box::new(StringColumnCarver::new("name".to_owned()))),
                ),
            ),
            TypeField::new(
                "expression".to_owned(),
                TypeFull::Type("String".to_owned()),
                FieldResolver::new(
                    vec![ExternalDependency::new("id".to_owned(), DependencyType::Id)],
                    vec![InternalDependency::new(
                        "expression".to_owned(),
                        DependencyType::String,
                        InternalDependencyResolver::ColumnGetter(ColumnGetter::new(
                            "actors".to_owned(),
                            "expression".to_owned(),
                        )),
                    )],
                    CarverOrPopulator::Carver(Box::new(StringColumnCarver::new(
                        "expression".to_owned(),
                    ))),
                ),
            ),
            TypeField::new(
                "favoriteActorOrDesigner".to_owned(),
                TypeFull::Type("ActorOrDesigner".to_owned()),
                FieldResolver::new(
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
                ),
            ),
        ],
        None,
        vec!["HasName".to_owned()],
    ));

    let designer_type = Type::Object(ObjectType::new(
        "Designer".to_owned(),
        vec![TypeField::new(
            "name".to_owned(),
            TypeFull::Type("String".to_owned()),
            FieldResolver::new(
                vec![ExternalDependency::new("id".to_owned(), DependencyType::Id)],
                vec![InternalDependency::new(
                    "name".to_owned(),
                    DependencyType::String,
                    InternalDependencyResolver::ColumnGetter(ColumnGetter::new(
                        "designers".to_owned(),
                        "name".to_owned(),
                    )),
                )],
                CarverOrPopulator::Carver(Box::new(StringColumnCarver::new("name".to_owned()))),
            ),
        )],
        None,
        vec!["HasName".to_owned()],
    ));

    let actor_or_designer = Union::new(
        "ActorOrDesigner".to_owned(),
        vec!["Actor".to_owned(), "Designer".to_owned()],
    );

    let (katie_id,): (Id,) = sqlx::query_as("SELECT id FROM actors WHERE name = 'Katie Cassidy'")
        .fetch_one(&db_pool)
        .await
        .unwrap();
    let (proenza_schouler_id,): (Id,) =
        sqlx::query_as("SELECT id FROM designers WHERE name = 'Proenza Schouler'")
            .fetch_one(&db_pool)
            .await
            .unwrap();

    let query_type = Type::Object(ObjectType::new(
        "Query".to_owned(),
        vec![
            TypeField::new(
                "actor".to_owned(),
                TypeFull::Type("Actor".to_owned()),
                // {
                //   external_dependencies => None,
                //   internal_dependencies => {"id" => Argument},
                //   populator => ValuePopulator("id"),
                // }
                FieldResolver::new(
                    vec![],
                    vec![InternalDependency::new(
                        "id".to_owned(),
                        DependencyType::Id,
                        InternalDependencyResolver::Argument(
                            ArgumentInternalDependencyResolver::new("id".to_owned()),
                        ),
                    )],
                    CarverOrPopulator::Populator(Box::new(ValuePopulator::new("id".to_owned()))),
                ),
            ),
            TypeField::new(
                "actors".to_owned(),
                TypeFull::List("Actor".to_owned()),
                // {
                //   external_dependencies => None,
                //   internal_dependencies => [
                //      column_fetcher_list!("actors", "id"),
                //   ],
                //   populator =>
                // }
                FieldResolver::new(
                    vec![],
                    vec![InternalDependency::new(
                        "ids".to_owned(),
                        DependencyType::ListOfIds,
                        InternalDependencyResolver::ColumnGetterList(ColumnGetterList::new(
                            "actors".to_owned(),
                            "id".to_owned(),
                        )),
                    )],
                    CarverOrPopulator::PopulatorList(Box::new(IdPopulatorList::new())),
                ),
            ),
            TypeField::new(
                "actorKatie".to_owned(),
                TypeFull::Type("Actor".to_owned()),
                // {
                //   external_dependencies => None,
                //   internal_dependencies => {"id" => LiteralValue(4)},
                //   populator => ValuePopulator("id"),
                // }
                FieldResolver::new(
                    vec![],
                    vec![InternalDependency::new(
                        "id".to_owned(),
                        DependencyType::Id,
                        InternalDependencyResolver::LiteralValue(
                            LiteralValueInternalDependencyResolver(DependencyValue::Id(katie_id)),
                        ),
                    )],
                    CarverOrPopulator::Populator(Box::new(ValuePopulator::new("id".to_owned()))),
                ),
            ),
            TypeField::new(
                "certainActorOrDesigner".to_owned(),
                TypeFull::Type("ActorOrDesigner".to_owned()),
                FieldResolver::new(
                    vec![],
                    vec![
                        InternalDependency::new(
                            "type".to_owned(),
                            DependencyType::String,
                            InternalDependencyResolver::LiteralValue(
                                LiteralValueInternalDependencyResolver(DependencyValue::String(
                                    "designers".to_owned(),
                                )),
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
                ),
            ),
            TypeField::new(
                "bestHasName".to_owned(),
                TypeFull::Type("HasName".to_owned()),
                FieldResolver::new(
                    vec![],
                    vec![
                        InternalDependency::new(
                            "type".to_owned(),
                            DependencyType::String,
                            InternalDependencyResolver::LiteralValue(
                                LiteralValueInternalDependencyResolver(DependencyValue::String(
                                    "actors".to_owned(),
                                )),
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
                ),
            ),
            TypeField::new(
                "actorsAndDesigners".to_owned(),
                TypeFull::List("ActorOrDesigner".to_owned()),
                FieldResolver::new(
                    vec![],
                    vec![
                        InternalDependency::new(
                            "actor_ids".to_owned(),
                            DependencyType::ListOfIds,
                            InternalDependencyResolver::ColumnGetterList(ColumnGetterList::new(
                                "actors".to_owned(),
                                "id".to_owned(),
                            )),
                        ),
                        InternalDependency::new(
                            "designer_ids".to_owned(),
                            DependencyType::ListOfIds,
                            InternalDependencyResolver::ColumnGetterList(ColumnGetterList::new(
                                "designers".to_owned(),
                                "id".to_owned(),
                            )),
                        ),
                    ],
                    CarverOrPopulator::UnionOrInterfaceTypePopulatorList(
                        Box::new(ActorsAndDesignersTypePopulator::new()),
                        Box::new(ActorsAndDesignersPopulator::new()),
                    ),
                ),
            ),
        ],
        Some(OperationType::Query),
        vec![],
    ));

    let schema = Schema::try_new(
        vec![query_type, actor_type, designer_type],
        vec![actor_or_designer],
        vec![has_name_interface],
    )?;

    let request = Request::new(Document::new(vec![
        // query {
        //   actorKatie {
        //     name
        //   }
        // }
        ExecutableDefinition::Operation(OperationDefinition::new(
            OperationType::Query,
            None,
            SelectionSet::new(vec![Selection::Field(SelectionField::new(
                None,
                "actorKatie".to_owned(),
                Some(SelectionSet::new(vec![Selection::Field(
                    SelectionField::new(None, "name".to_owned(), None),
                )])),
            ))]),
        )),
    ]));

    let response = schema.request(request, &db_pool).await;

    let json = json_from_response(&response);

    println!("actorKatie response: {}", pretty_print_json(&json));

    let request = Request::new(Document::new(vec![
        // query {
        //   actors {
        //     name
        //   }
        // }
        ExecutableDefinition::Operation(OperationDefinition::new(
            OperationType::Query,
            None,
            SelectionSet::new(vec![Selection::Field(SelectionField::new(
                None,
                "actors".to_owned(),
                Some(SelectionSet::new(vec![Selection::Field(
                    SelectionField::new(None, "name".to_owned(), None),
                )])),
            ))]),
        )),
    ]));

    let response = schema.request(request, &db_pool).await;

    let json = json_from_response(&response);

    println!("actors response: {}", pretty_print_json(&json));

    let request = Request::new(Document::new(vec![
        // query {
        //   actors {
        //     ...nameFragment
        //   }
        // }
        //
        // fragment nameFragment on Actor {
        //   name
        // }
        ExecutableDefinition::Operation(OperationDefinition::new(
            OperationType::Query,
            None,
            SelectionSet::new(vec![Selection::Field(SelectionField::new(
                None,
                "actors".to_owned(),
                Some(SelectionSet::new(vec![Selection::FragmentSpread(
                    FragmentSpread::new("nameFragment".to_owned()),
                )])),
            ))]),
        )),
        ExecutableDefinition::Fragment(FragmentDefinition::new(
            "nameFragment".to_owned(),
            "Actor".to_owned(),
            SelectionSet::new(vec![Selection::Field(SelectionField::new(
                None,
                "name".to_owned(),
                None,
            ))]),
        )),
    ]));

    let response = schema.request(request, &db_pool).await;

    let json = json_from_response(&response);

    println!("nameFragment response: {}", pretty_print_json(&json));

    let request = Request::new(Document::new(vec![
        // query {
        //   actors {
        //     ... {
        //       name
        //     }
        //   }
        // }
        ExecutableDefinition::Operation(OperationDefinition::new(
            OperationType::Query,
            None,
            SelectionSet::new(vec![Selection::Field(SelectionField::new(
                None,
                "actors".to_owned(),
                Some(SelectionSet::new(vec![Selection::InlineFragment(
                    InlineFragment::new(
                        None,
                        SelectionSet::new(vec![Selection::Field(SelectionField::new(
                            None,
                            "name".to_owned(),
                            None,
                        ))]),
                    ),
                )])),
            ))]),
        )),
    ]));

    let response = schema.request(request, &db_pool).await;

    let json = json_from_response(&response);

    println!("inline fragment response: {}", pretty_print_json(&json));

    let request = Request::new(Document::new(vec![
        // query {
        //   certainActorOrDesigner {
        //     ... on Actor {
        //       expression
        //     }
        //     ... on Designer {
        //       name
        //     }
        //   }
        // }
        ExecutableDefinition::Operation(OperationDefinition::new(
            OperationType::Query,
            None,
            SelectionSet::new(vec![Selection::Field(SelectionField::new(
                None,
                "certainActorOrDesigner".to_owned(),
                Some(SelectionSet::new(vec![
                    Selection::InlineFragment(InlineFragment::new(
                        Some("Actor".to_owned()),
                        SelectionSet::new(vec![Selection::Field(SelectionField::new(
                            None,
                            "expression".to_owned(),
                            None,
                        ))]),
                    )),
                    Selection::InlineFragment(InlineFragment::new(
                        Some("Designer".to_owned()),
                        SelectionSet::new(vec![Selection::Field(SelectionField::new(
                            None,
                            "name".to_owned(),
                            None,
                        ))]),
                    )),
                ])),
            ))]),
        )),
    ]));

    let response = schema.request(request, &db_pool).await;

    let json = json_from_response(&response);

    println!("union response: {}", pretty_print_json(&json));

    let request = Request::new(Document::new(vec![
        // query {
        //   actors {
        //     name
        //     expression
        //     favoriteActorOrDesigner {
        //       ... on Actor {
        //         expression
        //       }
        //       ... on Designer {
        //         name
        //       }
        //     }
        //   }
        // }
        ExecutableDefinition::Operation(OperationDefinition::new(
            OperationType::Query,
            None,
            SelectionSet::new(vec![Selection::Field(SelectionField::new(
                None,
                "actors".to_owned(),
                Some(SelectionSet::new(vec![
                    Selection::Field(SelectionField::new(None, "name".to_owned(), None)),
                    Selection::Field(SelectionField::new(None, "expression".to_owned(), None)),
                    Selection::Field(SelectionField::new(
                        None,
                        "favoriteActorOrDesigner".to_owned(),
                        Some(SelectionSet::new(vec![
                            Selection::InlineFragment(InlineFragment::new(
                                Some("Actor".to_owned()),
                                SelectionSet::new(vec![Selection::Field(SelectionField::new(
                                    None,
                                    "expression".to_owned(),
                                    None,
                                ))]),
                            )),
                            Selection::InlineFragment(InlineFragment::new(
                                Some("Designer".to_owned()),
                                SelectionSet::new(vec![Selection::Field(SelectionField::new(
                                    None,
                                    "name".to_owned(),
                                    None,
                                ))]),
                            )),
                        ])),
                    )),
                ])),
            ))]),
        )),
    ]));

    let response = schema.request(request, &db_pool).await;

    let json = json_from_response(&response);

    println!(
        "favorite actor or designer response: {}",
        pretty_print_json(&json)
    );

    let request = Request::new(Document::new(vec![
        // query {
        //   actors {
        //     favoriteActorOrDesigner {
        //       ... on HasName {
        //         name
        //       }
        //       ... on Actor {
        //         expression
        //       }
        //     }
        //   }
        //   bestHasName {
        //     name
        //   }
        // }
        ExecutableDefinition::Operation(OperationDefinition::new(
            OperationType::Query,
            None,
            SelectionSet::new(vec![
                Selection::Field(SelectionField::new(
                    None,
                    "actors".to_owned(),
                    Some(SelectionSet::new(vec![Selection::Field(
                        SelectionField::new(
                            None,
                            "favoriteActorOrDesigner".to_owned(),
                            Some(SelectionSet::new(vec![
                                Selection::InlineFragment(InlineFragment::new(
                                    Some("HasName".to_owned()),
                                    SelectionSet::new(vec![Selection::Field(SelectionField::new(
                                        None,
                                        "name".to_owned(),
                                        None,
                                    ))]),
                                )),
                                Selection::InlineFragment(InlineFragment::new(
                                    Some("Actor".to_owned()),
                                    SelectionSet::new(vec![Selection::Field(SelectionField::new(
                                        None,
                                        "expression".to_owned(),
                                        None,
                                    ))]),
                                )),
                            ])),
                        ),
                    )])),
                )),
                Selection::Field(SelectionField::new(
                    None,
                    "bestHasName".to_owned(),
                    Some(SelectionSet::new(vec![Selection::Field(
                        SelectionField::new(None, "name".to_owned(), None),
                    )])),
                )),
            ]),
        )),
    ]));

    let response = schema.request(request, &db_pool).await;

    let json = json_from_response(&response);

    println!("interface response: {}", pretty_print_json(&json));

    let request = Request::new(Document::new(vec![
        // query {
        //   actorsAndDesigners {
        //     ... on Actor {
        //       expression
        //     }
        //     ... on Designer {
        //       name
        //     }
        //   }
        // }
        ExecutableDefinition::Operation(OperationDefinition::new(
            OperationType::Query,
            None,
            SelectionSet::new(vec![Selection::Field(SelectionField::new(
                None,
                "actorsAndDesigners".to_owned(),
                Some(SelectionSet::new(vec![
                    Selection::InlineFragment(InlineFragment::new(
                        Some("Actor".to_owned()),
                        SelectionSet::new(vec![Selection::Field(SelectionField::new(
                            None,
                            "expression".to_owned(),
                            None,
                        ))]),
                    )),
                    Selection::InlineFragment(InlineFragment::new(
                        Some("Designer".to_owned()),
                        SelectionSet::new(vec![Selection::Field(SelectionField::new(
                            None,
                            "name".to_owned(),
                            None,
                        ))]),
                    )),
                ])),
            ))]),
        )),
    ]));

    let response = schema.request(request, &db_pool).await;

    let json = json_from_response(&response);

    println!("list union response: {}", pretty_print_json(&json));

    Ok(())
}

fn pretty_print_json(json: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
    serde_json::to_string_pretty(&parsed).unwrap()
}
