use thiserror::Error;

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

    pub async fn request(request: Request) -> Response {
        unimplemented!()
    }
}

pub enum Type {
    ObjectType(ObjectType),
}

impl Type {
    pub fn is_query_type(&self) -> bool {
        matches!(
            self,
            Self::ObjectType(type_) if type_.is_query_type()
        )
    }
}

pub enum OperationType {
    Query,
    Mutation,
    Subscription,
}

pub struct ObjectType {
    pub is_top_level_type: Option<OperationType>,
}

impl ObjectType {
    pub fn is_query_type(&self) -> bool {
        matches!(self.is_top_level_type, Some(OperationType::Query))
    }
}

pub struct Request {}

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
