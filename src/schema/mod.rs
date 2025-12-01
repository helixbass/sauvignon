use std::collections::{HashMap, HashSet};
use std::pin::Pin;

use sqlx::{Pool, Postgres};
use squalid::{OptionExt, _d};

use crate::{
    builtin_types, fields_in_progress_new, parse, CarverOrPopulator, DependencyType,
    DependencyValue, DummyUnionTypenameField, Error, ExternalDependencyValues, FieldPlan,
    FieldsInProgress, Id, InProgress, InProgressRecursing, InProgressRecursingList, IndexMap,
    Interface, InternalDependencyResolver, InternalDependencyValues, OperationType, Populator,
    PositionsTracker, QueryPlan, Request, Response, ResponseValue, ResponseValueOrInProgress,
    Result as SauvignonResult, Type, TypeInterface, Union, Value,
};

mod validation;
pub use validation::ValidationError;
use validation::ValidationRequestOrErrors;

pub struct Schema {
    pub types: HashMap<String, Type>,
    pub query_type_name: String,
    builtin_types: HashMap<String, Type>,
    pub unions: HashMap<String, Union>,
    pub interfaces: HashMap<String, Interface>,
    pub interface_all_concrete_types: HashMap<String, HashSet<String>>,
    pub dummy_union_typename_field: DummyUnionTypenameField,
}

impl Schema {
    pub fn try_new(
        types: Vec<Type>,
        unions: Vec<Union>,
        interfaces: Vec<Interface>,
    ) -> SauvignonResult<Self> {
        let query_type_index = types
            .iter()
            .position(|type_| type_.is_query_type())
            .ok_or_else(|| Error::NoQueryTypeSpecified)?;
        let query_type_name = types[query_type_index].name().to_owned();

        let interface_all_concrete_types = interfaces
            .iter()
            .map(|interface| {
                (
                    interface.name.clone(),
                    types
                        .iter()
                        .filter_map(|type_| match type_ {
                            Type::Object(object_type)
                                if object_type
                                    .implements
                                    .iter()
                                    .any(|implement| implement == &interface.name) =>
                            {
                                Some(object_type.name.clone())
                            }
                            _ => None,
                        })
                        .collect(),
                )
            })
            .collect();

        Ok(Self {
            types: types
                .into_iter()
                .map(|type_| (type_.name().to_owned(), type_))
                .collect(),
            query_type_name,
            builtin_types: builtin_types(),
            unions: unions
                .into_iter()
                .map(|union| (union.name.clone(), union))
                .collect(),
            interfaces: interfaces
                .into_iter()
                .map(|interface| (interface.name.clone(), interface))
                .collect(),
            interface_all_concrete_types,
            dummy_union_typename_field: _d(),
        })
    }

    pub async fn request(&self, request_str: &str, db_pool: &Pool<Postgres>) -> Response {
        let request = match parse(request_str.chars()) {
            Ok(request) => request,
            Err(error) => return Response::new(None, vec![error.into()]),
        };
        let validation_request_or_errors = self.validate(&request);
        if let ValidationRequestOrErrors::Errors(_) = validation_request_or_errors {
            let validation_errors = illicit::Layer::new()
                .offer(PositionsTracker::default())
                .enter(|| {
                    let request = parse(request_str.chars()).unwrap();
                    self.validate(&request).into_errors()
                });
            assert!(!validation_errors.is_empty());
            return Response::new(
                None,
                validation_errors.into_iter().map(Into::into).collect(),
            );
        }
        let response = compute_response(self, &request, db_pool).await;
        Response::new(Some(response), _d())
    }

    pub fn query_type(&self) -> &Type {
        &self.types[&self.query_type_name]
    }

    pub fn type_name_for_operation_type(&self, operation_type: OperationType) -> &str {
        match operation_type {
            OperationType::Query => &self.query_type_name,
            _ => unimplemented!(),
        }
    }

    pub fn maybe_type(&self, name: &str) -> Option<&Type> {
        self.types
            .get(name)
            .or_else(|| self.builtin_types.get(name))
    }

    pub fn type_(&self, name: &str) -> &Type {
        self.maybe_type(name).unwrap()
    }

    pub fn maybe_type_or_union_or_interface<'a>(
        &'a self,
        name: &str,
    ) -> Option<TypeOrUnionOrInterface<'a>> {
        if let Some(type_) = self.maybe_type(name) {
            return Some(TypeOrUnionOrInterface::Type(type_));
        }
        if let Some(union) = self.unions.get(name) {
            return Some(TypeOrUnionOrInterface::Union(union));
        }
        if let Some(interface) = self.interfaces.get(name) {
            return Some(TypeOrUnionOrInterface::Interface(interface));
        }
        None
    }

    pub fn type_or_union_or_interface<'a>(&'a self, name: &str) -> TypeOrUnionOrInterface<'a> {
        self.maybe_type_or_union_or_interface(name)
            .expect_else(|| format!("Unknown type/union/interface: {name}"))
    }

    pub fn all_concrete_type_names(
        &self,
        type_or_union_or_interface: &TypeOrUnionOrInterface,
    ) -> HashSet<String> {
        match type_or_union_or_interface {
            TypeOrUnionOrInterface::Type(type_) => [type_.name().to_owned()].into_iter().collect(),
            TypeOrUnionOrInterface::Union(union) => union.types.iter().cloned().collect(),
            TypeOrUnionOrInterface::Interface(interface) => {
                self.interface_all_concrete_types[&interface.name].clone()
            }
        }
    }

    pub fn all_concrete_type_names_for_type_or_union_or_interface(
        &self,
        name: &str,
    ) -> HashSet<String> {
        self.all_concrete_type_names(&self.type_or_union_or_interface(name))
    }
}

async fn compute_response(
    schema: &Schema,
    request: &Request,
    db_pool: &Pool<Postgres>,
) -> ResponseValue {
    let query_plan = QueryPlan::new(&request, schema);
    let response_in_progress = query_plan.initial_response_in_progress();
    let mut fields_in_progress = response_in_progress.fields;
    loop {
        let ret = progress_fields(fields_in_progress, db_pool, schema).await;
        let is_done = ret.0;
        fields_in_progress = ret.1;
        if is_done {
            return fields_in_progress.into();
        }
    }
}

fn progress_fields<'a>(
    fields_in_progress: FieldsInProgress<'a>,
    db_pool: &'a Pool<Postgres>,
    schema: &'a Schema,
) -> Pin<Box<dyn Future<Output = (bool, FieldsInProgress<'a>)> + 'a>> {
    Box::pin(async move {
        let is_done = fields_in_progress
            .values()
            .all(|field| matches!(field, ResponseValueOrInProgress::ResponseValue(_)));
        if is_done {
            return (true, fields_in_progress);
        }

        let mut progressed = IndexMap::new();
        for (field_name, response_value_or_in_progress) in fields_in_progress {
            progressed.insert(
                field_name,
                match response_value_or_in_progress {
                    ResponseValueOrInProgress::ResponseValue(response_value) => {
                        ResponseValueOrInProgress::ResponseValue(response_value)
                    }
                    ResponseValueOrInProgress::InProgress(InProgress {
                        field_plan,
                        external_dependency_values,
                    }) => {
                        let internal_dependency_values = populate_internal_dependencies(
                            field_plan,
                            &external_dependency_values,
                            db_pool,
                            schema,
                        )
                        .await;
                        match &field_plan.field_type.resolver.carver_or_populator {
                            CarverOrPopulator::Populator(populator) => {
                                to_recursing_after_populating(
                                    &external_dependency_values,
                                    &internal_dependency_values,
                                    populator,
                                    field_plan.field_type.type_.name(),
                                    field_plan,
                                )
                            }
                            CarverOrPopulator::PopulatorList(populator) => {
                                let populated = populator.populate(
                                    &external_dependency_values,
                                    &internal_dependency_values,
                                );
                                let type_name = field_plan.field_type.type_.name();
                                let fields_in_progress = populated
                                    .iter()
                                    .map(|populated| {
                                        fields_in_progress_new(
                                            &field_plan.selection_set_by_type.as_ref().unwrap()
                                                [type_name],
                                            &populated,
                                        )
                                    })
                                    .collect();
                                ResponseValueOrInProgress::InProgressRecursingList(
                                    InProgressRecursingList::new(
                                        field_plan,
                                        populated,
                                        fields_in_progress,
                                    ),
                                )
                            }
                            CarverOrPopulator::Carver(carver) => {
                                ResponseValueOrInProgress::ResponseValue(carver.carve(
                                    &external_dependency_values,
                                    &internal_dependency_values,
                                ))
                            }
                            CarverOrPopulator::UnionOrInterfaceTypePopulator(
                                type_populator,
                                populator,
                            ) => {
                                let type_name = type_populator.populate(
                                    &external_dependency_values,
                                    &internal_dependency_values,
                                );
                                to_recursing_after_populating(
                                    &external_dependency_values,
                                    &internal_dependency_values,
                                    populator,
                                    &type_name,
                                    field_plan,
                                )
                            }
                            CarverOrPopulator::UnionOrInterfaceTypePopulatorList(
                                type_populator,
                                populator,
                            ) => {
                                let type_names = type_populator.populate(
                                    &external_dependency_values,
                                    &internal_dependency_values,
                                );
                                let populated = populator.populate(
                                    &external_dependency_values,
                                    &internal_dependency_values,
                                );
                                assert!(type_names.len() == populated.len());
                                let fields_in_progress = populated
                                    .iter()
                                    .zip(type_names)
                                    .map(|(populated, type_name)| {
                                        fields_in_progress_new(
                                            &field_plan.selection_set_by_type.as_ref().unwrap()
                                                [&type_name],
                                            populated,
                                        )
                                    })
                                    .collect();
                                ResponseValueOrInProgress::InProgressRecursingList(
                                    InProgressRecursingList::new(
                                        field_plan,
                                        populated,
                                        fields_in_progress,
                                    ),
                                )
                            }
                        }
                    }
                    ResponseValueOrInProgress::InProgressRecursing(InProgressRecursing {
                        field_plan,
                        populated,
                        selection,
                    }) => {
                        let (is_done, fields_in_progress) =
                            progress_fields(selection, db_pool, schema).await;

                        if is_done {
                            ResponseValueOrInProgress::ResponseValue(fields_in_progress.into())
                        } else {
                            ResponseValueOrInProgress::InProgressRecursing(InProgressRecursing {
                                field_plan,
                                populated,
                                selection: fields_in_progress,
                            })
                        }
                    }
                    ResponseValueOrInProgress::InProgressRecursingList(
                        InProgressRecursingList {
                            field_plan,
                            populated,
                            selections,
                        },
                    ) => {
                        let mut progressed = vec![];
                        let mut are_all_done = true;
                        for selection in selections {
                            let (is_done, fields_in_progress) =
                                progress_fields(selection, db_pool, schema).await;
                            if !is_done {
                                are_all_done = false;
                            }
                            progressed.push(fields_in_progress);
                        }
                        if are_all_done {
                            ResponseValueOrInProgress::ResponseValue(progressed.into())
                        } else {
                            ResponseValueOrInProgress::InProgressRecursingList(
                                InProgressRecursingList {
                                    field_plan,
                                    populated,
                                    selections: progressed,
                                },
                            )
                        }
                    }
                },
            );
        }

        (false, progressed)
    })
}

async fn populate_internal_dependencies(
    field_plan: &FieldPlan<'_>,
    external_dependency_values: &ExternalDependencyValues,
    db_pool: &Pool<Postgres>,
    schema: &Schema,
) -> InternalDependencyValues {
    let mut ret = InternalDependencyValues::default();
    for internal_dependency in field_plan.field_type.resolver.internal_dependencies.iter() {
        ret.insert(
            internal_dependency.name.clone(),
            match &internal_dependency.resolver {
                InternalDependencyResolver::ColumnGetter(column_getter) => {
                    let row_id = match external_dependency_values.get("id").unwrap() {
                        DependencyValue::Id(id) => id,
                        _ => unreachable!(),
                    };
                    match internal_dependency.type_ {
                        DependencyType::Id => {
                            // TODO: should check that table names and column names can never be SQL injection?
                            let query = format!(
                                "SELECT {} FROM {} WHERE id = $1",
                                column_getter.column_name, column_getter.table_name
                            );
                            let (column_value,): (Id,) = sqlx::query_as(&query)
                                .bind(row_id)
                                .fetch_one(db_pool)
                                .await
                                .unwrap();
                            DependencyValue::Id(column_value)
                        }
                        DependencyType::String => {
                            // TODO: should check that table names and column names can never be SQL injection?
                            let query = format!(
                                "SELECT {} FROM {} WHERE id = $1",
                                column_getter.column_name, column_getter.table_name
                            );
                            let (column_value,): (String,) = sqlx::query_as(&query)
                                .bind(row_id)
                                .fetch_one(db_pool)
                                .await
                                .unwrap();
                            DependencyValue::String(column_value)
                        }
                        _ => unimplemented!(),
                    }
                }
                InternalDependencyResolver::LiteralValue(literal_value) => literal_value.0.clone(),
                InternalDependencyResolver::ColumnGetterList(column_getter_list) => {
                    // TODO: same as above, sql injection?
                    let query = format!(
                        "SELECT {} FROM {}",
                        column_getter_list.column_name, column_getter_list.table_name
                    );
                    let rows = sqlx::query_as::<_, (Id,)>(&query)
                        .fetch_all(db_pool)
                        .await
                        .unwrap();
                    DependencyValue::List(
                        rows.into_iter()
                            .map(|(column_value,)| DependencyValue::Id(column_value))
                            .collect(),
                    )
                }
                InternalDependencyResolver::IntrospectionTypeInterfaces => {
                    let type_name = match external_dependency_values.get("name").unwrap() {
                        DependencyValue::String(name) => name,
                        _ => unreachable!(),
                    };
                    DependencyValue::List(
                        schema
                            .maybe_type(type_name)
                            .filter(|type_| matches!(type_, Type::Object(_)))
                            .map(|type_| {
                                type_
                                    .as_object()
                                    .implements
                                    .iter()
                                    .map(|implement| DependencyValue::String(implement.clone()))
                                    .collect()
                            })
                            .or_else(|| {
                                schema.interfaces.get(type_name).map(|interface| {
                                    interface
                                        .implements
                                        .iter()
                                        .map(|implement| DependencyValue::String(implement.clone()))
                                        .collect()
                                })
                            })
                            .unwrap_or_default(),
                    )
                }
                InternalDependencyResolver::Argument(argument_resolver) => {
                    let argument = field_plan
                        .arguments
                        .as_ref()
                        .unwrap()
                        .get(&argument_resolver.name)
                        .unwrap();
                    match (internal_dependency.type_, &argument.value) {
                        (DependencyType::Id, Value::Int(argument_value)) => {
                            DependencyValue::Id(*argument_value)
                        }
                        (DependencyType::String, Value::String(argument_value)) => {
                            DependencyValue::String(argument_value.clone())
                        }
                        // TODO: truly unreachable?
                        _ => unreachable!(),
                    }
                }
            },
        )
        .unwrap();
    }

    ret
}

fn to_recursing_after_populating<'a>(
    external_dependency_values: &ExternalDependencyValues,
    internal_dependency_values: &InternalDependencyValues,
    populator: &Box<dyn Populator>,
    resolved_concrete_type_name: &str,
    field_plan: &'a FieldPlan<'a>,
) -> ResponseValueOrInProgress<'a> {
    let populated = populator.populate(&external_dependency_values, &internal_dependency_values);
    let fields_in_progress = fields_in_progress_new(
        &field_plan.selection_set_by_type.as_ref().unwrap()[resolved_concrete_type_name],
        &populated,
    );
    ResponseValueOrInProgress::InProgressRecursing(InProgressRecursing::new(
        field_plan,
        populated,
        fields_in_progress,
    ))
}

#[derive(Copy, Clone)]
pub enum TypeOrUnionOrInterface<'a> {
    Type(&'a Type),
    Union(&'a Union),
    Interface(&'a Interface),
}

impl<'a> TypeOrUnionOrInterface<'a> {
    pub fn name(&self) -> &str {
        match self {
            Self::Type(type_) => type_.name(),
            Self::Union(union) => &union.name,
            Self::Interface(interface) => &interface.name,
        }
    }
}
