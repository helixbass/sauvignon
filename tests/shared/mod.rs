use std::collections::HashMap;

use sqlx::{postgres::PgPoolOptions, Pool, Postgres};

use sauvignon::{
    schema, Carver, CarverOrPopulator, ExternalDependencyValues, Id, InternalDependencyValues,
    PopulatorListInterface, ResponseValue, Schema, UnionOrInterfaceTypePopulatorList,
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
    let (katie_id,): (Id,) = sqlx::query_as("SELECT id FROM actors WHERE name = 'Katie Cassidy'")
        .fetch_one(db_pool)
        .await
        .unwrap();
    let (proenza_schouler_id,): (Id,) =
        sqlx::query_as("SELECT id FROM designers WHERE name = 'Proenza Schouler'")
            .fetch_one(db_pool)
            .await
            .unwrap();

    Ok(schema! {
        types => [
            Actor => {
                fields => [
                    name => string_column()
                    expression => string_column(),
                    favoriteDesigner => belongs_to(
                        type => Designer
                    )
                    favoriteActorOrDesigner => belongs_to(
                        type => ActorOrDesigner
                        polymorphic => true
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
                    id => literal_value(id => katie_id),
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
                    id => literal_value(id => proenza_schouler_id)
                ]
            }
            bestHasName => {
                type => HasName!
                internal_dependencies => [
                    type => literal_value("actors")
                    id => literal_value(id => katie_id)
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
            bestCanadianCity => {
                type => CanadianCity!
                internal_dependencies => [
                    value => literal_value("VANCOUVER")
                ]
            }
            canadianCityQuote => {
                type => String!
                params => [
                    city => CanadianCity!
                ]
                carver => custom {
                    CarverOrPopulator::Carver(Box::new(CanadianCityQuoteCarver::default()))
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
        enums => [
            CanadianCity => [VANCOUVER, CORNER_BROOK, QUEBEC, MONTREAL]
        ]
    })
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
