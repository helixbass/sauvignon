use thiserror::Error;

mod operation;
mod request;
mod types;

pub use crate::operation::OperationType;
pub use crate::request::{
    Document, ExecutableDefinition, Field as SelectionField, OperationDefinition, Request,
    Selection, SelectionSet,
};
pub use crate::types::{
    BuiltInScalarType, Field as TypeField, ObjectType, ScalarType, StringType, Type,
};

#[derive(Error, Debug)]
pub enum Error {
    #[error("must provide query type")]
    NoQueryTypeSpecified,
}

type SauvignonResult<TSuccess> = Result<TSuccess, Error>;

pub struct Schema {
    pub types: Vec<Type>,
    query_type_index: usize,
}

impl Schema {
    pub fn try_new(types: Vec<Type>) -> SauvignonResult<Self> {
        let query_type_index = types
            .iter()
            .position(|type_| type_.is_query_type())
            .ok_or_else(|| Error::NoQueryTypeSpecified)?;

        Ok(Self {
            types,
            query_type_index,
        })
    }

    pub async fn request(&self, request: Request) -> Response {
        unimplemented!()
    }
}

pub struct Response {}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn it_works() {
//         let result = add(2, 2);
//         assert_eq!(result, 4);
//     }
// }
