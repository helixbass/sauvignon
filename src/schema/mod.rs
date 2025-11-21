use std::collections::HashMap;

use crate::{
    builtin_types, fields_in_progress_new, CarverOrPopulator, Error, ExternalDependencyValues,
    FieldsInProgress, InProgress, InProgressRecursing, IndexMap, InternalDependencyValues,
    QueryPlan, Request, Response, ResponseValue, ResponseValueOrInProgress,
    Result as SauvignonResult, Type, TypeInterface,
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
        let response = compute_response(self, &request);
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

fn compute_response(schema: &Schema, request: &Request) -> ResponseValue {
    let query_plan = QueryPlan::new(&request, schema);
    let response_in_progress = query_plan.initial_response_in_progress();
    let mut fields_in_progress = response_in_progress.fields;
    loop {
        let ret = progress_fields(fields_in_progress);
        let is_done = ret.0;
        fields_in_progress = ret.1;
        if is_done {
            return fields_in_progress.into();
        }
    }
}

fn progress_fields<'a>(fields_in_progress: FieldsInProgress<'a>) -> (bool, FieldsInProgress<'a>) {
    let is_done = fields_in_progress
        .values()
        .all(|field| matches!(field, ResponseValueOrInProgress::ResponseValue(_)));
    if is_done {
        return (true, fields_in_progress);
    }

    (
        false,
        IndexMap::from_iter(fields_in_progress.into_iter().map(
            |(field_name, response_value_or_in_progress)| {
                (
                    field_name,
                    match response_value_or_in_progress {
                        ResponseValueOrInProgress::ResponseValue(response_value) => {
                            ResponseValueOrInProgress::ResponseValue(response_value)
                        }
                        ResponseValueOrInProgress::InProgress(InProgress {
                            field_plan,
                            external_dependency_values,
                        }) => {
                            let internal_dependency_values =
                                populate_internal_dependencies(&external_dependency_values);
                            match &field_plan.field_type.resolver.carver_or_populator {
                                CarverOrPopulator::Populator(populator) => {
                                    let mut populated = ExternalDependencyValues::default();
                                    populator.populate(
                                        &mut populated,
                                        &external_dependency_values,
                                        &internal_dependency_values,
                                    );
                                    ResponseValueOrInProgress::InProgressRecursing(
                                        InProgressRecursing::new(
                                            field_plan,
                                            populated,
                                            fields_in_progress_new(
                                                field_plan.selection_set.as_ref().unwrap(),
                                            ),
                                        ),
                                    )
                                }
                                CarverOrPopulator::Carver(carver) => {
                                    ResponseValueOrInProgress::ResponseValue(carver.carve(
                                        &external_dependency_values,
                                        &internal_dependency_values,
                                    ))
                                }
                            }
                        }
                        ResponseValueOrInProgress::InProgressRecursing(InProgressRecursing {
                            field_plan,
                            populated,
                            selection,
                        }) => {
                            unimplemented!()
                        }
                    },
                )
            },
        )),
    )
}

fn populate_internal_dependencies(
    external_dependency_values: &ExternalDependencyValues,
) -> InternalDependencyValues {
    unimplemented!()
}
