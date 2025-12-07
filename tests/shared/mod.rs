use std::collections::HashMap;

use sqlx::{postgres::PgPoolOptions, Pool, Postgres};

use sauvignon::{
    ArgumentInternalDependencyResolver, Carver, CarverOrPopulator, ColumnGetter, ColumnGetterList,
    DependencyType, DependencyValue, Enum, ExternalDependency, ExternalDependencyValues,
    FieldResolver, Id, InterfaceBuilder, InterfaceField, InternalDependency,
    InternalDependencyResolver, InternalDependencyValues, LiteralValueInternalDependencyResolver,
    ObjectTypeBuilder, OperationType, Param, PopulatorListInterface, ResponseValue, Schema,
    StringCarver, Type, TypeDepluralizer, TypeFieldBuilder, TypeFull, Union,
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

impl PopulatorListInterface for ActorsAndDesignersPopulator {
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

static CANADIAN_CITIES: [&'static str; 4] = ["VANCOUVER", "CORNER_BROOK", "QUEBEC", "MONTREAL"];

pub struct CanadianCityQuoteCarver {
    pub quotes: HashMap<&'static str, String>,
}

impl Default for CanadianCityQuoteCarver {
    fn default() -> Self {
        Self {
            quotes: CANADIAN_CITIES
                .iter()
                .map(|city| {
                    (
                        *city,
                        match *city {
                            "VANCOUVER" => "We're the best".to_owned(),
                            _ => "We're the worst".to_owned(),
                        },
                    )
                })
                .collect(),
        }
    }
}

impl Carver for CanadianCityQuoteCarver {
    fn carve(
        &self,
        _external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> ResponseValue {
        ResponseValue::EnumValue(
            self.quotes[&**internal_dependencies.get("city").unwrap().as_string()].clone(),
        )
    }
}

pub async fn get_schema(db_pool: &Pool<Postgres>) -> anyhow::Result<Schema> {
    let has_name_interface = InterfaceBuilder::default()
        .name("HasName")
        .fields(vec![InterfaceField::new(
            "name".to_owned(),
            TypeFull::Type("String".to_owned()),
            [],
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
                            ValuesPopulator::new([(
                                "favorite_actor_or_designer_id".to_owned(),
                                "id".to_owned(),
                            )])
                            .into(),
                        ),
                    ))
                    .build()
                    .unwrap(),
                TypeFieldBuilder::default()
                    .name("favoriteDesigner")
                    .type_(TypeFull::Type("Designer".to_owned()))
                    .resolver(FieldResolver::new(
                        vec![ExternalDependency::new("id".to_owned(), DependencyType::Id)],
                        vec![InternalDependency::new(
                            "favorite_designer_id".to_owned(),
                            DependencyType::Id,
                            InternalDependencyResolver::ColumnGetter(ColumnGetter::new(
                                "actors".to_owned(),
                                "favorite_designer_id".to_owned(),
                            )),
                        )],
                        CarverOrPopulator::Populator(
                            ValuesPopulator::new([(
                                "favorite_designer_id".to_owned(),
                                "id".to_owned(),
                            )])
                            .into(),
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

    let canadian_city = Type::Enum(Enum::new(
        "CanadianCity".to_owned(),
        CANADIAN_CITIES.iter().map(|city| (*city).to_owned()),
    ));

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
                        CarverOrPopulator::Populator(ValuePopulator::new("id".to_owned()).into()),
                    ))
                    .params([Param::new(
                        "id".to_owned(),
                        TypeFull::NonNull(Box::new(TypeFull::Type("Id".to_owned()))),
                    )])
                    .build()
                    .unwrap(),
                TypeFieldBuilder::default()
                    .name("actors")
                    .type_(TypeFull::List(Box::new(TypeFull::Type("Actor".to_owned()))))
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
                        CarverOrPopulator::PopulatorList(
                            ValuePopulatorList::new("id".to_owned()).into(),
                        ),
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
                        CarverOrPopulator::Populator(ValuePopulator::new("id".to_owned()).into()),
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
                            ValuePopulator::new("id".to_owned()).into(),
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
                            ValuePopulator::new("id".to_owned()).into(),
                        ),
                    ))
                    .build()
                    .unwrap(),
                TypeFieldBuilder::default()
                    .name("actorsAndDesigners")
                    .type_(TypeFull::List(Box::new(TypeFull::Type(
                        "ActorOrDesigner".to_owned(),
                    ))))
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
                TypeFieldBuilder::default()
                    .name("bestCanadianCity")
                    .type_(TypeFull::Type("CanadianCity".to_owned()))
                    .resolver(FieldResolver::new(
                        vec![],
                        vec![InternalDependency::new(
                            "value".to_owned(),
                            DependencyType::String,
                            InternalDependencyResolver::LiteralValue(
                                LiteralValueInternalDependencyResolver(DependencyValue::String(
                                    "VANCOUVER".to_owned(),
                                )),
                            ),
                        )],
                        CarverOrPopulator::Carver(Box::new(StringCarver::new("value".to_owned()))),
                    ))
                    .build()
                    .unwrap(),
                TypeFieldBuilder::default()
                    .name("canadianCityQuote")
                    .type_(TypeFull::Type("String".to_owned()))
                    .resolver(FieldResolver::new(
                        vec![],
                        vec![InternalDependency::new(
                            "city".to_owned(),
                            DependencyType::String,
                            InternalDependencyResolver::Argument(
                                ArgumentInternalDependencyResolver::new("city".to_owned()),
                            ),
                        )],
                        CarverOrPopulator::Carver(Box::new(CanadianCityQuoteCarver::default())),
                    ))
                    .params([Param::new(
                        "city".to_owned(),
                        TypeFull::NonNull(Box::new(TypeFull::Type("CanadianCity".to_owned()))),
                    )])
                    .build()
                    .unwrap(),
            ])
            .is_top_level_type(OperationType::Query)
            .build()
            .unwrap(),
    );

    Ok(Schema::try_new(
        vec![query_type, actor_type, designer_type, canadian_city],
        vec![actor_or_designer],
        vec![has_name_interface],
    )?)
}

pub async fn get_db_pool() -> anyhow::Result<Pool<Postgres>> {
    let db_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://sauvignon:password@localhost/sauvignon")
        .await?;

    Ok(db_pool)
}

pub fn pretty_print_json(json: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
    serde_json::to_string_pretty(&parsed).unwrap()
}
