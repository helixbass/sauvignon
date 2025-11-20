use sauvignon::{
    BuiltInScalarType, DependencyType, Document, Error as SauvignonError, ExecutableDefinition,
    ExternalDependency, InternalDependency, ObjectType, OperationDefinition, OperationType,
    Request, ScalarType, Schema, Selection, SelectionField, SelectionSet, StringType, Type,
    TypeField,
};

#[tokio::main]
async fn main() -> Result<(), SauvignonError> {
    let actor_type = Type::Object(ObjectType::new(
        "Actor".to_owned(),
        vec![TypeField::new(
            "name".to_owned(),
            Type::Scalar(ScalarType::BuiltIn(BuiltInScalarType::String(
                StringType::new(),
            ))),
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
                    DependencyType::Omg,
                )],
            ),
        )],
        None,
    ));

    let query_type = Type::Object(ObjectType::new(
        "Query".to_owned(),
        vec![TypeField::new(
            "actor".to_owned(),
            actor_type,
            // external_dependencies => None,
            // internal_dependencies => ,
            // populator => {"id" => 4}
        )],
        Some(OperationType::Query),
    ));

    let schema = Schema::try_new(vec![query_type])?;

    let response = schema
        .request(Request::new(Document::new(vec![
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
        ])))
        .await;

    Ok(())
}
