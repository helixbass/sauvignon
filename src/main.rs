use std::collections::HashMap;

use sauvignon::{
    ArgumentInternalDependencyResolver, CarverOrPopulator, ColumnGetter, ColumnGetterList,
    DependencyType, Document, Error as SauvignonError, ExecutableDefinition, ExternalDependency,
    FieldResolver, IdPopulator, InternalDependency, InternalDependencyResolver, ObjectType,
    OperationDefinition, OperationType, Request, Schema, Selection, SelectionField, SelectionSet,
    StringColumnCarver, Type, TypeField, TypeFull,
};

#[tokio::main]
async fn main() -> Result<(), SauvignonError> {
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

    let query_type = Type::Object(ObjectType::new(
        "Query".to_owned(),
        vec![
            TypeField::new(
                "actor".to_owned(),
                TypeFull::Type("Actor".to_owned()),
                // external_dependencies => None,
                // internal_dependencies => ,
                // populator => {"id" => 4}
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
                    CarverOrPopulator::Populator(Box::new(IdPopulator::new())),
                ),
            ),
        ],
        Some(OperationType::Query),
    ));

    let schema = Schema::try_new(vec![query_type, actor_type])?;

    let request = Request::new(Document::new(vec![
        // query {
        //   actor {
        //     name
        //   }
        // }
        ExecutableDefinition::Operation(OperationDefinition::new(
            OperationType::Query,
            None,
            SelectionSet::new(vec![Selection::Field(SelectionField::new(
                None,
                "actor".to_owned(),
                Some(SelectionSet::new(vec![Selection::Field(
                    SelectionField::new(None, "name".to_owned(), None),
                )])),
            ))]),
        )),
    ]));

    let response = schema.request(request).await;

    Ok(())
}
