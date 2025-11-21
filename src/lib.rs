use thiserror::Error;

mod any_hash_map;
mod dependencies;
mod operation;
mod request;
mod resolve;
mod response;
mod types;

pub use indexmap::IndexMap;

pub use crate::any_hash_map::AnyHashMap;
pub use crate::dependencies::{
    ArgumentInternalDependencyResolver, ColumnGetter, ColumnGetterList, DependencyType,
    ExternalDependency, ExternalDependencyValue, ExternalDependencyValues, InternalDependency,
    InternalDependencyResolver, InternalDependencyValue, InternalDependencyValues,
};
pub use crate::operation::OperationType;
pub use crate::request::{
    Document, ExecutableDefinition, Field as SelectionField, OperationDefinition, Request,
    Selection, SelectionSet,
};
pub use crate::resolve::{
    Carver, CarverOrPopulator, FieldResolver, IdPopulator, StringColumnCarver,
};
pub use crate::response::{Response, ResponseValue};
pub use crate::types::{
    BuiltInScalarType, Field as TypeField, ObjectType, ScalarType, StringType, Type, TypeFull,
};

#[derive(Error, Debug)]
pub enum Error {
    #[error("must provide query type")]
    NoQueryTypeSpecified,
    #[error("dependency already populated: `{0}`")]
    DependencyAlreadyPopulated(String),
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

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn it_works() {
//         let result = add(2, 2);
//         assert_eq!(result, 4);
//     }
// }
