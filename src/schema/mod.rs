use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use rkyv::{rancor, util::AlignedVec};
use smol_str::SmolStr;
use squalid::{OptionExt, _d};
use tracing::{instrument, trace, trace_span};

use crate::{
    builtin_types, get_hash, parse, produce_response, Database, DatabaseInterface, Document,
    DummyUnionTypenameField, Error, Interface, OperationType, PositionsTracker, QueryPlan, Request,
    Response, ResponseValue, Result as SauvignonResult, Type, TypeInterface, Union,
};

mod sync;
mod validation;

use sync::compute_sync_response;
pub use validation::ValidationError;
use validation::ValidationRequestOrErrors;

pub struct Schema {
    pub types: HashMap<SmolStr, Type>,
    pub query_type_name: SmolStr,
    builtin_types: HashMap<SmolStr, Type>,
    pub unions: HashMap<SmolStr, Union>,
    pub interfaces: HashMap<SmolStr, Interface>,
    pub interface_all_concrete_types: HashMap<SmolStr, HashSet<SmolStr>>,
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
        let query_type_name = types[query_type_index].name().into();

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
                .map(|type_| (type_.name().into(), type_))
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

    #[instrument(level = "debug", skip(self, database))]
    pub async fn request(&self, document_str: &str, database: &Database) -> Response {
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
        let is_database_sync = database.is_sync();
        compute_response(self, &request, (database, is_database_sync))
            .await
            .into()
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
    ) -> HashSet<SmolStr> {
        match type_or_union_or_interface {
            TypeOrUnionOrInterface::Type(type_) => [type_.name().into()].into_iter().collect(),
            TypeOrUnionOrInterface::Union(union) => union.types.iter().cloned().collect(),
            TypeOrUnionOrInterface::Interface(interface) => {
                self.interface_all_concrete_types[&interface.name].clone()
            }
        }
    }

    pub fn all_concrete_type_names_for_type_or_union_or_interface(
        &self,
        name: &str,
    ) -> HashSet<SmolStr> {
        self.all_concrete_type_names(&self.type_or_union_or_interface(name))
    }
}

#[instrument(level = "debug", skip(schema, request, database))]
async fn compute_response(
    schema: &Schema,
    request: &Request,
    database: (&Database, bool),
) -> ResponseValue {
    let query_plan = QueryPlan::new(&request, schema, database.0.column_tokens());
    if database.1 {
        return compute_sync_response(schema, database.0, &query_plan);
    }
    produce_response(schema, database.0, &query_plan).await
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
