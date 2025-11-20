use sauvignon::{
    BuiltInScalarType, Error as SauvignonError, Field, ObjectType, OperationType, ScalarType,
    Schema, StringType, Type,
};

#[tokio::main]
async fn main() -> Result<(), SauvignonError> {
    let actor_type = Type::Object(ObjectType::new(
        "Actor".to_owned(),
        vec![Field::new(
            "name".to_owned(),
            Type::Scalar(ScalarType::BuiltIn(BuiltInScalarType::String(
                StringType::new(),
            ))),
        )],
        None,
    ));

    let query_type = Type::Object(ObjectType::new(
        "Query".to_owned(),
        vec![Field::new("actor".to_owned(), actor_type)],
        Some(OperationType::Query),
    ));

    let schema = Schema::try_new(vec![query_type])?;

    let response = schema.request().await;

    Ok(())
}
