use std::collections::HashMap;

use crate::{
    builtin_types, Error, QueryPlan, Request, Response, Result as SauvignonResult, Type,
    TypeInterface,
};

pub struct Schema {
    pub types: HashMap<String, Type>,
    pub query_type_name: String,
    builtin_types: HashMap<String, Type>,
}

impl Schema {
    pub fn try_new(types: Vec<Type>) -> SauvignonResult<Self> {
        let query_type_index = types
            .iter()
            .position(|type_| type_.is_query_type())
            .ok_or_else(|| Error::NoQueryTypeSpecified)?;
        let query_type_name = types[query_type_index].name().to_owned();

        Ok(Self {
            types: HashMap::from_iter(
                types
                    .into_iter()
                    .map(|type_| (type_.name().to_owned(), type_)),
            ),
            query_type_name,
            builtin_types: builtin_types(),
        })
    }

    pub async fn request(&self, request: Request) -> Response {
        let query_plan = QueryPlan::new(&request, self);
        let mut is_complete = false;
        let mut response_in_progress = query_plan.initial_response_in_progress();
        unimplemented!()
    }

    pub fn query_type(&self) -> &Type {
        &self.types[&self.query_type_name]
    }

    pub fn get_type(&self, name: &str) -> &Type {
        self.types
            .get(name)
            .or_else(|| self.builtin_types.get(name))
            .unwrap()
    }
}
