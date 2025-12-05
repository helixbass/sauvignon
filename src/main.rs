use sqlx::postgres::PgPoolOptions;

use sauvignon::{
    json_from_response, ArgumentInternalDependencyResolver, CarverOrPopulator, ColumnGetter,
    ColumnGetterList, DependencyType, DependencyValue, Document, ExecutableDefinition,
    ExternalDependency, FieldResolver, FragmentDefinition, FragmentSpread, Id, IdPopulator,
    IdPopulatorList, InlineFragment, InternalDependency, InternalDependencyResolver,
    LiteralValueInternalDependencyResolver, ObjectType, OperationDefinition, OperationType,
    Request, Schema, Selection, SelectionField, SelectionSet, StringColumnCarver, Type, TypeField,
    TypeFull,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect("postgres://sauvignon:password@localhost/sauvignon")
        .await?;

    let actor_type = Type::Object(ObjectType::new(
        "Actor".to_owned(),
        vec![TypeField::new(
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
        )],
        None,
    ));

    let (katie_id,): (Id,) = sqlx::query_as("SELECT id FROM actors WHERE name = 'Katie Cassidy'")
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
                //   populator => IdPopulator,
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
                    CarverOrPopulator::Populator(Box::new(IdPopulator::new())),
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
                //   populator => IdPopulator,
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
                    CarverOrPopulator::Populator(Box::new(IdPopulator::new())),
                ),
            ),
        ],
        Some(OperationType::Query),
    ));

    let schema = Schema::try_new(vec![query_type, actor_type])?;

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

    Ok(())
}

fn pretty_print_json(json: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
    serde_json::to_string_pretty(&parsed).unwrap()
}
