use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::sync::RwLock;

use itertools::Itertools;
use rkyv::{rancor, util::AlignedVec};
use sql_query_builder::Select;
use sqlx::{Pool, Postgres, Row};
use squalid::{EverythingExt, OptionExt, _d};
use tracing::{instrument, trace, trace_span, Instrument};

use crate::{
    builtin_types, fields_in_progress_new, get_hash, parse, pluralize, CarverOrPopulator,
    DependencyType, DependencyValue, Document, DummyUnionTypenameField, Error,
    ExternalDependencyValues, FieldPlan, FieldResolver, FieldsInProgress, Id, InProgress,
    InProgressRecursing, InProgressRecursingList, IndexMap, Interface, InternalDependency,
    InternalDependencyResolver, InternalDependencyValues, OperationType, Populator,
    PopulatorInterface, PopulatorList, PopulatorListInterface, PositionsTracker, QueryPlan,
    Request, Response, ResponseValue, ResponseValueOrInProgress, Result as SauvignonResult, Type,
    TypeInterface, Union, Value,
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
    pub cached_validated_documents: RwLock<HashMap<u64, AlignedVec>>,
}

impl Schema {
    #[instrument(level = "trace", skip(types, unions, interfaces))]
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
            cached_validated_documents: _d(),
        })
    }

    #[instrument(level = "debug", skip(self, db_pool))]
    pub async fn request(&self, document_str: &str, db_pool: &Pool<Postgres>) -> Response {
        let document_str_hash = get_hash(document_str);
        let cached_validated_document = self
            .cached_validated_documents
            .read()
            .unwrap()
            .get(&document_str_hash)
            .map(|cached_validated_document| unsafe {
                rkyv::from_bytes_unchecked::<Document, rancor::Error>(cached_validated_document)
                    .unwrap()
            });
        trace!(
            succeeded = cached_validated_document.is_some(),
            "Tried to get cached validated document",
        );
        let request = match cached_validated_document {
            Some(cached_validated_document) => Request::new(cached_validated_document),
            None => {
                let request = match parse(document_str.chars()) {
                    Ok(request) => request,
                    Err(_) => {
                        let _ = trace_span!("re-parsing with position tracking").entered();
                        let parse_error = illicit::Layer::new()
                            .offer(PositionsTracker::default())
                            .enter(|| parse(document_str.chars()).unwrap_err());
                        return vec![parse_error.into()].into();
                    }
                };
                let validation_request_or_errors = self.validate(&request);
                if let ValidationRequestOrErrors::Errors(_) = validation_request_or_errors {
                    let _ = trace_span!("re-validating with position tracking").entered();
                    let validation_errors = illicit::Layer::new()
                        .offer(PositionsTracker::default())
                        .enter(|| {
                            let request = parse(document_str.chars()).unwrap();
                            self.validate(&request).into_errors()
                        });
                    assert!(!validation_errors.is_empty());
                    return validation_errors
                        .into_iter()
                        .map(Into::into)
                        .collect::<Vec<_>>()
                        .into();
                }
                self.cached_validated_documents.write().unwrap().insert(
                    document_str_hash,
                    rkyv::to_bytes::<rancor::Error>(&request.document).unwrap(),
                );
                trace!("cached validated document");
                request
            }
        };
        compute_response(self, &request, db_pool).await.into()
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

#[instrument(level = "debug", skip(schema, request, db_pool))]
async fn compute_response(
    schema: &Schema,
    request: &Request,
    db_pool: &Pool<Postgres>,
) -> ResponseValue {
    let query_plan = QueryPlan::new(&request, schema);
    if let Some(list_query_response) = maybe_optimize_list_query(&query_plan, db_pool).await {
        return list_query_response;
    }
    if let Some(list_query_response) =
        maybe_optimize_list_sub_belongs_to_query(&query_plan, db_pool).await
    {
        return list_query_response;
    }
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

#[instrument(level = "debug", skip(query_plan, db_pool))]
async fn maybe_optimize_list_sub_belongs_to_query(
    query_plan: &QueryPlan<'_>,
    db_pool: &Pool<Postgres>,
) -> Option<ResponseValue> {
    if query_plan.field_plans.len() != 1 {
        return None;
    }
    let field_plan = query_plan.field_plans.values().next().unwrap();
    let (table_name, id_column_name) = maybe_list_of_ids_field(field_plan)?;
    let selection_set =
        field_plan
            .selection_set_by_type
            .as_ref()
            .unwrap()
            .thrush(|selection_set_by_type| {
                if selection_set_by_type.len() != 1 {
                    return None;
                }
                Some(selection_set_by_type.values().next().unwrap())
            })?;
    if selection_set.len() != 1 {
        return None;
    }
    let sub_field_plan = selection_set.values().next().unwrap();
    let foreign_key_column_name = maybe_belongs_to(sub_field_plan, table_name, id_column_name)?;
    let sub_selection_set = sub_field_plan
        .selection_set_by_type
        .as_ref()
        .unwrap()
        .thrush(|selection_set_by_type| {
            if selection_set_by_type.len() != 1 {
                return None;
            }
            Some(selection_set_by_type.values().next().unwrap())
        })?;
    let (belongs_to_table_name, belongs_to_column_names_to_select) =
        maybe_column_names_to_select_bootstrap_table_name(sub_selection_set, id_column_name)?;
    // TODO: SQL injection?
    let mut query = Select::default().from(table_name).inner_join(&format!("{belongs_to_table_name} on {table_name}.{foreign_key_column_name} = {belongs_to_table_name}.id"));
    for belongs_to_column_name in &belongs_to_column_names_to_select {
        query = query.select(&format!("{belongs_to_table_name}.{belongs_to_column_name}"));
    }
    Some(ResponseValue::Map(
        [(
            field_plan.name.clone(),
            ResponseValue::List(
                sqlx::query(&query.as_string())
                    .fetch_all(db_pool)
                    .instrument(trace_span!("fetch belongs-to"))
                    .await
                    .unwrap()
                    .into_iter()
                    .map(|row| {
                        ResponseValue::Map(
                            [(
                                sub_field_plan.name.clone(),
                                ResponseValue::Map(
                                    belongs_to_column_names_to_select
                                        .iter()
                                        .enumerate()
                                        .map(|(index, belongs_to_column_name)| {
                                            (
                                                (*belongs_to_column_name).to_owned(),
                                                ResponseValue::String(row.get(index)),
                                            )
                                        })
                                        .collect(),
                                ),
                            )]
                            .into_iter()
                            .collect(),
                        )
                    })
                    .collect(),
            ),
        )]
        .into_iter()
        .collect(),
    ))
}

#[instrument(level = "trace", skip(field_plan))]
fn maybe_belongs_to<'a>(
    field_plan: &'a FieldPlan<'a>,
    table_name: &str,
    id_column_name: &str,
) -> Option<&'a str> {
    let foreign_key_column_name = maybe_column_getter_field(
        &field_plan.field_type.resolver,
        table_name,
        id_column_name,
        DependencyType::Id,
    )?;
    match &field_plan.field_type.resolver.carver_or_populator {
        CarverOrPopulator::Populator(Populator::Values(values_populator))
            if values_populator.keys.len() == 1 =>
        {
            let only_values_key_mapping = values_populator.keys.iter().next().unwrap();
            if only_values_key_mapping.0 != foreign_key_column_name {
                return None;
            }
            if only_values_key_mapping.1 != "id" {
                return None;
            }
            Some(foreign_key_column_name)
        }
        _ => None,
    }
}

#[instrument(level = "trace", skip(selection_set))]
fn maybe_column_names_to_select_bootstrap_table_name<'a>(
    selection_set: &'a IndexMap<String, FieldPlan<'a>>,
    id_column_name: &str,
) -> Option<(&'a str, Vec<&'a str>)> {
    let mut table_name: Option<&'a str> = _d();
    let mut ret: Vec<&'a str> = _d();
    for (_, field_plan) in selection_set {
        ret.push({
            let column_name = match table_name {
                None => {
                    let (table_name_, column_name) =
                        maybe_column_getter_field_bootstrap_table_name(
                            &field_plan.field_type.resolver,
                            id_column_name,
                        )?;
                    table_name = Some(table_name_);
                    column_name
                }
                Some(table_name) => maybe_column_getter_field(
                    &field_plan.field_type.resolver,
                    table_name,
                    id_column_name,
                    DependencyType::String,
                )?,
            };
            if field_plan.field_type.name != column_name {
                return None;
            }
            column_name
        });
    }
    Some((table_name.unwrap(), ret))
}

#[instrument(level = "debug", skip(query_plan, db_pool))]
async fn maybe_optimize_list_query(
    query_plan: &QueryPlan<'_>,
    db_pool: &Pool<Postgres>,
) -> Option<ResponseValue> {
    if query_plan.field_plans.len() != 1 {
        return None;
    }
    let field_plan = query_plan.field_plans.values().next().unwrap();
    // TODO: I've currently baked in the assumption I believe that the
    // internal dependendency has the same plural name as the column
    // (eg "ids" and "id")
    let (table_name, id_column_name) = maybe_list_of_ids_field(field_plan)?;
    let selection_set =
        field_plan
            .selection_set_by_type
            .as_ref()
            .unwrap()
            .thrush(|selection_set_by_type| {
                if selection_set_by_type.len() != 1 {
                    return None;
                }
                Some(selection_set_by_type.values().next().unwrap())
            })?;
    let column_names_to_select =
        maybe_column_names_to_select(selection_set, table_name, id_column_name)?;
    // TODO: SQL injection?
    let mut query = Select::default().from(table_name).select(id_column_name);
    for column_name in &column_names_to_select {
        query = query.select(column_name);
    }
    Some(ResponseValue::Map(
        [(
            field_plan.name.clone(),
            ResponseValue::List(
                sqlx::query(&query.as_string())
                    .fetch_all(db_pool)
                    .instrument(trace_span!("fetch list"))
                    .await
                    .unwrap()
                    .into_iter()
                    .map(|row| {
                        ResponseValue::Map(
                            column_names_to_select
                                .iter()
                                .enumerate()
                                .map(|(index, column_name)| {
                                    (
                                        (*column_name).to_owned(),
                                        ResponseValue::String(row.get(index + 1)),
                                    )
                                })
                                .collect(),
                        )
                    })
                    .collect(),
            ),
        )]
        .into_iter()
        .collect(),
    ))
}

#[instrument(level = "trace", skip(selection_set))]
fn maybe_column_names_to_select<'a>(
    selection_set: &'a IndexMap<String, FieldPlan<'a>>,
    table_name: &str,
    id_column_name: &str,
) -> Option<Vec<&'a str>> {
    let mut ret: Vec<&'a str> = _d();
    for (_, field_plan) in selection_set {
        ret.push({
            let column_name = maybe_column_getter_field(
                &field_plan.field_type.resolver,
                table_name,
                id_column_name,
                DependencyType::String,
            )?;
            if field_plan.field_type.name != column_name {
                return None;
            }
            column_name
        });
    }
    Some(ret)
}

#[instrument(level = "trace", skip(resolver))]
fn maybe_column_getter_field_bootstrap_table_name<'a>(
    resolver: &'a FieldResolver,
    id_column_name: &str,
) -> Option<(&'a str, &'a str)> {
    if resolver.external_dependencies.len() != 1 {
        return None;
    }
    if !has_id_external_dependency_only(resolver, id_column_name) {
        return None;
    }
    maybe_column_getter_internal_dependency_bootstrap_table_name(resolver)
}

#[instrument(level = "trace", skip(resolver))]
fn maybe_column_getter_field<'a>(
    resolver: &'a FieldResolver,
    table_name: &str,
    id_column_name: &str,
    dependency_type: DependencyType,
) -> Option<&'a str> {
    if resolver.external_dependencies.len() != 1 {
        return None;
    }
    if !has_id_external_dependency_only(resolver, id_column_name) {
        return None;
    }
    maybe_column_getter_internal_dependency(resolver, table_name, dependency_type)
}

#[instrument(level = "trace", skip(resolver))]
fn maybe_column_getter_internal_dependency_bootstrap_table_name<'a>(
    resolver: &'a FieldResolver,
) -> Option<(&'a str, &'a str)> {
    if resolver.internal_dependencies.len() != 1 {
        return None;
    }
    let internal_dependency = &resolver.internal_dependencies[0];
    if internal_dependency.type_ != DependencyType::String {
        return None;
    }
    match &internal_dependency.resolver {
        InternalDependencyResolver::ColumnGetter(column_getter) => {
            Some((&column_getter.table_name, &column_getter.column_name))
        }
        _ => None,
    }
}

#[instrument(level = "trace", skip(resolver))]
fn maybe_column_getter_internal_dependency<'a>(
    resolver: &'a FieldResolver,
    table_name: &str,
    dependency_type: DependencyType,
) -> Option<&'a str> {
    if resolver.internal_dependencies.len() != 1 {
        return None;
    }
    let internal_dependency = &resolver.internal_dependencies[0];
    if internal_dependency.type_ != dependency_type {
        return None;
    }
    match &internal_dependency.resolver {
        InternalDependencyResolver::ColumnGetter(column_getter)
            if column_getter.table_name == table_name =>
        {
            Some(&column_getter.column_name)
        }
        _ => None,
    }
}

#[instrument(level = "trace", skip(resolver))]
fn has_id_external_dependency_only(resolver: &FieldResolver, id_column_name: &str) -> bool {
    if resolver.external_dependencies.len() != 1 {
        return false;
    }
    if resolver.external_dependencies[0].name != id_column_name {
        return false;
    }
    if resolver.external_dependencies[0].type_ != DependencyType::Id {
        return false;
    }
    true
}

#[instrument(level = "trace", skip(field_plan))]
fn maybe_list_of_ids_field<'a>(field_plan: &'a FieldPlan<'a>) -> Option<(&'a str, &'a str)> {
    let (table_name, id_column_name) = maybe_list_of_ids_internal_dependencies(
        &field_plan.field_type.resolver.internal_dependencies,
    )?;
    match &field_plan.field_type.resolver.carver_or_populator {
        CarverOrPopulator::PopulatorList(PopulatorList::Value(value_populator_list))
            if value_populator_list.singular == id_column_name =>
        {
            Some((table_name, id_column_name))
        }
        _ => None,
    }
}

#[instrument(level = "trace", skip(internal_dependencies))]
fn maybe_list_of_ids_internal_dependencies(
    internal_dependencies: &Vec<InternalDependency>,
) -> Option<(&str, &str)> {
    if internal_dependencies.len() != 1 {
        return None;
    }
    let internal_dependency = &internal_dependencies[0];
    if internal_dependency.type_ != DependencyType::ListOfIds {
        return None;
    }
    match &internal_dependency.resolver {
        InternalDependencyResolver::ColumnGetterList(column_getter_list) => {
            if !column_getter_list.wheres.is_empty() {
                return None;
            }
            if pluralize(&column_getter_list.column_name) != internal_dependency.name {
                return None;
            }
            Some((
                &column_getter_list.table_name,
                &column_getter_list.column_name,
            ))
        }
        _ => None,
    }
}

#[instrument(level = "debug", skip(fields_in_progress, db_pool, schema))]
fn progress_fields<'a>(
    fields_in_progress: FieldsInProgress<'a>,
    db_pool: &'a Pool<Postgres>,
    schema: &'a Schema,
) -> Pin<Box<dyn Future<Output = (bool, FieldsInProgress<'a>)> + 'a + Send>> {
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

#[instrument(
    level = "trace",
    skip(field_plan, external_dependency_values, db_pool, schema)
)]
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
                                .instrument(trace_span!("fetch ID column"))
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
                                .instrument(trace_span!("fetch string column"))
                                .await
                                .unwrap();
                            DependencyValue::String(column_value)
                        }
                        DependencyType::OptionalFloat => {
                            // TODO: should check that table names and column names can never be SQL injection?
                            let query = format!(
                                "SELECT {} FROM {} WHERE id = $1",
                                column_getter.column_name, column_getter.table_name
                            );
                            let (column_value,): (Option<f64>,) = sqlx::query_as(&query)
                                .bind(row_id)
                                .fetch_one(db_pool)
                                .instrument(trace_span!("fetch string column"))
                                .await
                                .unwrap();
                            match column_value {
                                None => DependencyValue::Null,
                                Some(column_value) => DependencyValue::Float(column_value),
                            }
                        }
                        _ => unimplemented!(),
                    }
                }
                InternalDependencyResolver::LiteralValue(literal_value) => literal_value.0.clone(),
                InternalDependencyResolver::ColumnGetterList(column_getter_list) => {
                    // TODO: same as above, sql injection?
                    let query = format!(
                        "SELECT {} FROM {}{}",
                        column_getter_list.column_name,
                        column_getter_list.table_name,
                        if column_getter_list.wheres.is_empty() {
                            "".to_owned()
                        } else {
                            format!(
                                " WHERE {}",
                                column_getter_list
                                    .wheres
                                    .iter()
                                    .enumerate()
                                    .map(|(index, where_)| {
                                        format!("{} = ${}", where_.column_name, index + 1)
                                    })
                                    .collect::<String>()
                            )
                        }
                    );
                    let mut query = sqlx::query_as::<_, (Id,)>(&query);
                    for _where in &column_getter_list.wheres {
                        // TODO: this is punting on where's specifying
                        // values
                        query = match external_dependency_values.get("id").unwrap() {
                            DependencyValue::Id(id) => query.bind(id),
                            DependencyValue::String(str) => query.bind(str),
                            _ => unimplemented!(),
                        };
                    }
                    let rows = query
                        .fetch_all(db_pool)
                        .instrument(trace_span!("fetch column list"))
                        .await
                        .unwrap();
                    DependencyValue::List(
                        rows.into_iter()
                            .map(|(column_value,)| DependencyValue::Id(column_value))
                            .collect(),
                    )
                }
                InternalDependencyResolver::IntrospectionTypeInterfaces => {
                    let _ = trace_span!("resolve introspection type interfaces").entered();
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
                            // TODO: this needs to be optional for
                            // things other than object types and interfaces
                            .unwrap(),
                    )
                }
                InternalDependencyResolver::IntrospectionTypePossibleTypes => {
                    let _ = trace_span!("resolve introspection type possible types").entered();
                    let type_name = match external_dependency_values.get("name").unwrap() {
                        DependencyValue::String(name) => name,
                        _ => unreachable!(),
                    };
                    DependencyValue::List(
                        schema
                            .interface_all_concrete_types
                            .get(type_name)
                            .map(|all_concrete_type_names| {
                                all_concrete_type_names
                                    .into_iter()
                                    .sorted()
                                    .map(|concrete_type_name| {
                                        DependencyValue::String(concrete_type_name.clone())
                                    })
                                    .collect()
                            })
                            .or_else(|| {
                                schema.unions.get(type_name).map(|union| {
                                    union
                                        .types
                                        .iter()
                                        .map(|concrete_type_name| {
                                            DependencyValue::String(concrete_type_name.clone())
                                        })
                                        .collect()
                                })
                            })
                            // TODO: this needs to be optional for
                            // things other than interfaces and unions
                            .unwrap(),
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
                        (DependencyType::String, Value::EnumVariant(argument_value)) => {
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

#[instrument(
    level = "trace",
    skip(
        external_dependency_values,
        internal_dependency_values,
        populator,
        field_plan
    )
)]
fn to_recursing_after_populating<'a>(
    external_dependency_values: &ExternalDependencyValues,
    internal_dependency_values: &InternalDependencyValues,
    populator: &Populator,
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
