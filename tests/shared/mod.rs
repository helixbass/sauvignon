use std::str::FromStr;

use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use strum::{EnumString, VariantNames};

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

#[derive(Copy, Clone, Debug, PartialEq, Eq, VariantNames, EnumString)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
enum CanadianCity {
    Vancouver,
    CornerBrook,
    Quebec,
    Montreal,
}

impl CanadianCity {
    pub fn quote(&self) -> &'static str {
        match self {
            Self::Vancouver => "We're the best",
            _ => "We're the worst",
        }
    }
}

#[derive(Default)]
pub struct CanadianCityQuoteCarver {}

impl Carver for CanadianCityQuoteCarver {
    fn carve(
        &self,
        _external_dependencies: &ExternalDependencyValues,
        internal_dependencies: &InternalDependencyValues,
    ) -> ResponseValue {
        ResponseValue::EnumValue(
            CanadianCity::from_str(internal_dependencies.get("city").unwrap().as_string())
                .unwrap()
                .quote()
                .to_owned(),
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
                    favoriteOfActors => has_many(
                        type => Actor
                        foreign_key => favorite_designer_id
                    )
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
            designers => {
                type => [Designer!]!
                internal_dependencies => [
                    ids => id_column_list()
                ]
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
            CanadianCity,
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
