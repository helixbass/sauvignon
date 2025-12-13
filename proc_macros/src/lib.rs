use std::collections::HashSet;

use heck::ToSnakeCase;
use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use shared::pluralize;
use squalid::{OptionExtDefault, OptionExtIterator, _d};
use syn::{
    braced, bracketed, parenthesized,
    parse::{Parse, ParseBuffer, ParseStream, Result},
    parse_macro_input,
    spanned::Spanned,
    ExprBlock, Ident, LitBool, LitInt, LitStr, Token,
};

struct Schema {
    pub types: Vec<Type>,
    pub query: Vec<Field>,
    pub interfaces: Option<Vec<Interface>>,
    pub unions: Option<Vec<Union>>,
    pub enums: Option<Vec<Enum>>,
}

impl Schema {
    pub fn process(self) -> SchemaProcessed {
        let all_union_or_interface_type_names = self
            .interfaces
            .as_ref()
            .map(|interfaces| {
                interfaces
                    .into_iter()
                    .map(|interface| interface.name.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
            .into_iter()
            .chain(
                self.unions
                    .as_ref()
                    .map(|unions| {
                        unions
                            .into_iter()
                            .map(|union| union.name.clone())
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
                    .into_iter(),
            )
            .collect::<HashSet<_>>();
        let all_enum_names = self
            .enums
            .as_ref()
            .map(|enums| {
                enums
                    .into_iter()
                    .map(|enum_| enum_.name.to_string())
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default();
        SchemaProcessed {
            types: self
                .types
                .into_iter()
                .map(|type_| type_.process(&all_union_or_interface_type_names, &all_enum_names))
                .collect(),
            query: self
                .query
                .into_iter()
                .map(|field| {
                    field.process(None, &all_union_or_interface_type_names, &all_enum_names)
                })
                .collect(),
            interfaces: self.interfaces,
            unions: self.unions,
            enums: self.enums,
        }
    }
}

impl Parse for Schema {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut types: Option<Vec<Type>> = _d();
        let mut query: Option<Vec<Field>> = _d();
        let mut interfaces: Option<Vec<Interface>> = _d();
        let mut unions: Option<Vec<Union>> = _d();
        let mut enums: Option<Vec<Enum>> = _d();

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=>]>()?;
            match &*key.to_string() {
                "types" => {
                    assert!(types.is_none(), "Already saw 'types' key");
                    let types_content;
                    bracketed!(types_content in input);
                    let types = types.populate_default();
                    while !types_content.is_empty() {
                        types.push(types_content.parse()?);
                        types_content.parse::<Option<Token![,]>>()?;
                    }
                }
                "query" => {
                    assert!(query.is_none(), "Already saw 'query' key");
                    let query_content;
                    bracketed!(query_content in input);
                    let query = query.populate_default();
                    while !query_content.is_empty() {
                        query.push(query_content.parse()?);
                        query_content.parse::<Option<Token![,]>>()?;
                    }
                }
                "interfaces" => {
                    assert!(interfaces.is_none(), "Already saw 'interfaces' key");
                    let interfaces_content;
                    bracketed!(interfaces_content in input);
                    let interfaces = interfaces.populate_default();
                    while !interfaces_content.is_empty() {
                        interfaces.push(interfaces_content.parse()?);
                        interfaces_content.parse::<Option<Token![,]>>()?;
                    }
                }
                "unions" => {
                    assert!(unions.is_none(), "Already saw 'unions' key");
                    let unions_content;
                    bracketed!(unions_content in input);
                    let unions = unions.populate_default();
                    while !unions_content.is_empty() {
                        unions.push(unions_content.parse()?);
                        unions_content.parse::<Option<Token![,]>>()?;
                    }
                }
                "enums" => {
                    assert!(enums.is_none(), "Already saw 'enums' key");
                    let enums_content;
                    bracketed!(enums_content in input);
                    let enums = enums.populate_default();
                    while !enums_content.is_empty() {
                        enums.push(enums_content.parse()?);
                        enums_content.parse::<Option<Token![,]>>()?;
                    }
                }
                key => return Err(input.error(format!("Unexpected key `{key}`"))),
            }
        }

        Ok(Self {
            types: types.expect("Didn't see `types`"),
            query: query.expect("Didn't see `query`"),
            interfaces,
            unions,
            enums,
        })
    }
}

struct SchemaProcessed {
    pub types: Vec<TypeProcessed>,
    pub query: Vec<FieldProcessed>,
    pub interfaces: Option<Vec<Interface>>,
    pub unions: Option<Vec<Union>>,
    pub enums: Option<Vec<Enum>>,
}

struct Type {
    pub name: String,
    pub fields: Vec<Field>,
    pub implements: Option<Vec<String>>,
}

impl Type {
    pub fn process(
        self,
        all_union_or_interface_type_names: &HashSet<String>,
        all_enum_names: &HashSet<String>,
    ) -> TypeProcessed {
        TypeProcessed {
            name: self.name.clone(),
            fields: self
                .fields
                .into_iter()
                .map(|field| {
                    field.process(
                        Some(&self.name),
                        all_union_or_interface_type_names,
                        all_enum_names,
                    )
                })
                .collect(),
            implements: self.implements,
        }
    }
}

impl Parse for Type {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;
        input.parse::<Token![=>]>()?;
        let type_content;
        braced!(type_content in input);
        let mut fields: Option<Vec<Field>> = _d();
        let mut implements: Option<Vec<String>> = _d();
        while !type_content.is_empty() {
            let key: Ident = type_content.parse()?;
            type_content.parse::<Token![=>]>()?;
            match &*key.to_string() {
                "fields" => {
                    assert!(fields.is_none(), "Already saw 'fields' key");
                    let fields_content;
                    bracketed!(fields_content in type_content);
                    let fields = fields.populate_default();
                    while !fields_content.is_empty() {
                        fields.push(fields_content.parse()?);
                        fields_content.parse::<Option<Token![,]>>()?;
                    }
                }
                "implements" => {
                    assert!(implements.is_none(), "Already saw 'implements' key");
                    let implements_content;
                    bracketed!(implements_content in type_content);
                    let implements = implements.populate_default();
                    while !implements_content.is_empty() {
                        implements.push(implements_content.parse::<Ident>()?.to_string());
                        implements_content.parse::<Option<Token![,]>>()?;
                    }
                }
                key => return Err(type_content.error(format!("Unexpected key `{key}`"))),
            }
            type_content.parse::<Option<Token![,]>>()?;
        }

        Ok(Self {
            name: name.to_string(),
            fields: fields.expect("Didn't see `fields`"),
            implements,
        })
    }
}

struct TypeProcessed {
    pub name: String,
    pub fields: Vec<FieldProcessed>,
    pub implements: Option<Vec<String>>,
}

struct Field {
    pub name: String,
    pub value: FieldValue,
}

impl Field {
    pub fn process(
        self,
        parent_type_name: Option<&str>,
        all_union_or_interface_type_names: &HashSet<String>,
        all_enum_names: &HashSet<String>,
    ) -> FieldProcessed {
        FieldProcessed {
            name: self.name,
            value: self.value.process(
                parent_type_name,
                all_union_or_interface_type_names,
                all_enum_names,
            ),
        }
    }
}

impl Parse for Field {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;
        input.parse::<Token![=>]>()?;
        let value: FieldValue = input.parse()?;

        Ok(Self {
            name: name.to_string(),
            value,
        })
    }
}

struct FieldProcessed {
    pub name: String,
    pub value: FieldValueProcessed,
}

impl ToTokens for FieldProcessed {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = &self.name;
        match &self.value {
            FieldValueProcessed::StringColumn {
                table_name,
            } => {
                quote! {
                    ::sauvignon::TypeFieldBuilder::default()
                        .name(#name)
                        .type_(::sauvignon::TypeFull::Type("String".to_owned()))
                        .resolver(::sauvignon::FieldResolver::new(
                            vec![::sauvignon::ExternalDependency::new("id".to_owned(), ::sauvignon::DependencyType::Id)],
                            vec![::sauvignon::InternalDependency::new(
                                #name.to_owned(),
                                ::sauvignon::DependencyType::String,
                                ::sauvignon::InternalDependencyResolver::ColumnGetter(::sauvignon::ColumnGetter::new(
                                    #table_name.to_owned(),
                                    #name.to_owned(),
                                )),
                            )],
                            ::sauvignon::CarverOrPopulator::Carver(::std::boxed::Box::new(::sauvignon::StringCarver::new(#name.to_owned()))),
                        ))
                        .build()
                        .unwrap()
                }
            }
            FieldValueProcessed::Object {
                type_,
                internal_dependencies,
                params,
                maybe_type_kind,
                carver_or_populator,
            } => {
                let carver_or_populator = match carver_or_populator {
                    Some(carver_or_populator) => quote! {
                        #carver_or_populator
                    },
                    None => match type_.is_list_type() {
                        true => quote! {
                            ::sauvignon::CarverOrPopulator::PopulatorList(::sauvignon::ValuePopulatorList::new("id".to_owned()).into())
                        },
                        false => match maybe_type_kind {
                            None => quote! {
                                ::sauvignon::CarverOrPopulator::Populator(::sauvignon::ValuePopulator::new("id".to_owned()).into())
                            },
                            Some(TypeKind::UnionOrInterface) => quote! {
                                ::sauvignon::CarverOrPopulator::UnionOrInterfaceTypePopulator(
                                    Box::new(::sauvignon::TypeDepluralizer::new()),
                                    ::sauvignon::ValuePopulator::new("id".to_owned()).into(),
                                )
                            },
                            Some(TypeKind::Enum) => {
                                let only_internal_dependency_name = {
                                    assert!(matches!(
                                        internal_dependencies.as_ref(),
                                        Some(internal_dependencies) if internal_dependencies.len() == 1
                                    ));
                                    internal_dependencies.as_ref().unwrap()[0].name.clone()
                                };
                                quote! {
                                    ::sauvignon::CarverOrPopulator::Carver(Box::new(::sauvignon::StringCarver::new(#only_internal_dependency_name.to_owned())))
                                }
                            }
                        },
                    },
                };
                let internal_dependencies = params.as_ref().map(|params| {
                    params.into_iter().map(|param| {
                        InternalDependencyProcessed {
                            name: param.name.clone(),
                            type_: InternalDependencyTypeProcessed::Param(if param.is_id() {
                                DependencyType::Id
                            } else {
                                DependencyType::String
                            }),
                        }
                    })
                }).unwrap_or_empty().chain(
                    internal_dependencies.as_ref().map(|internal_dependencies| {
                        internal_dependencies.into_iter().cloned()
                    }).unwrap_or_empty()
                );
                let params = match params {
                    None => quote! { },
                    Some(params) => {
                        let params = params.into_iter().map(|param| quote! { #param });
                        quote! {
                            .params([
                                #(#params),*
                            ])
                        }
                    }
                };
                quote! {
                    ::sauvignon::TypeFieldBuilder::default()
                        .name(#name)
                        .type_(#type_)
                        .resolver(::sauvignon::FieldResolver::new(
                            vec![],
                            vec![#(#internal_dependencies),*],
                            #carver_or_populator,
                        ))
                        #params
                        .build()
                        .unwrap()
                }
            }
            FieldValueProcessed::BelongsTo {
                type_,
                self_table_name,
                polymorphic,
            } => {
                let self_belongs_to_foreign_key_column_name =
                    format!("{}_id", name.to_snake_case());
                match polymorphic {
                    false => quote! {
                        ::sauvignon::TypeFieldBuilder::default()
                            .name(#name)
                            .type_(::sauvignon::TypeFull::Type(#type_.to_owned()))
                            .resolver(::sauvignon::FieldResolver::new(
                                vec![::sauvignon::ExternalDependency::new("id".to_owned(), ::sauvignon::DependencyType::Id)],
                                vec![::sauvignon::InternalDependency::new(
                                    #self_belongs_to_foreign_key_column_name.to_owned(),
                                    ::sauvignon::DependencyType::Id,
                                    ::sauvignon::InternalDependencyResolver::ColumnGetter(::sauvignon::ColumnGetter::new(
                                        #self_table_name.to_owned(),
                                        #self_belongs_to_foreign_key_column_name.to_owned(),
                                    )),
                                )],
                                ::sauvignon::CarverOrPopulator::Populator(::sauvignon::ValuesPopulator::new([(
                                    #self_belongs_to_foreign_key_column_name.to_owned(),
                                    "id".to_owned(),
                                )]).into()),
                            ))
                            .build()
                            .unwrap()
                    },
                    true => {
                        let self_belongs_to_foreign_key_type_column_name =
                            format!("{}_type", name.to_snake_case());
                        quote! {
                            ::sauvignon::TypeFieldBuilder::default()
                                .name(#name)
                                .type_(::sauvignon::TypeFull::Type(#type_.to_owned()))
                                .resolver(::sauvignon::FieldResolver::new(
                                    vec![::sauvignon::ExternalDependency::new("id".to_owned(), ::sauvignon::DependencyType::Id)],
                                    vec![
                                        ::sauvignon::InternalDependency::new(
                                            "type".to_owned(),
                                            ::sauvignon::DependencyType::String,
                                            ::sauvignon::InternalDependencyResolver::ColumnGetter(::sauvignon::ColumnGetter::new(
                                                #self_table_name.to_owned(),
                                                #self_belongs_to_foreign_key_type_column_name.to_owned(),
                                            )),
                                        ),
                                        ::sauvignon::InternalDependency::new(
                                            #self_belongs_to_foreign_key_column_name.to_owned(),
                                            ::sauvignon::DependencyType::Id,
                                            ::sauvignon::InternalDependencyResolver::ColumnGetter(::sauvignon::ColumnGetter::new(
                                                #self_table_name.to_owned(),
                                                #self_belongs_to_foreign_key_column_name.to_owned(),
                                            )),
                                        ),
                                    ],
                                    ::sauvignon::CarverOrPopulator::UnionOrInterfaceTypePopulator(
                                        ::std::boxed::Box::new(::sauvignon::TypeDepluralizer::new()),
                                        ::sauvignon::ValuesPopulator::new([(
                                            #self_belongs_to_foreign_key_column_name.to_owned(),
                                            "id".to_owned(),
                                        )]).into()),
                                ))
                                .build()
                                .unwrap()
                        }
                    }
                }
            }
            FieldValueProcessed::HasMany {
                type_,
                foreign_key,
                through,
                through_self_column_name,
                through_other_column_name,
            } => {
                match (foreign_key, through) {
                    (Some(foreign_key), None) => {
                        let foreign_key_table_name = pluralize(&type_.to_snake_case());

                        quote! {
                            ::sauvignon::TypeFieldBuilder::default()
                                .name(#name)
                                .type_(::sauvignon::TypeFull::List(::std::boxed::Box::new(::sauvignon::TypeFull::Type(#type_.to_owned()))))
                                .resolver(::sauvignon::FieldResolver::new(
                                    vec![::sauvignon::ExternalDependency::new("id".to_owned(), ::sauvignon::DependencyType::Id)],
                                    vec![::sauvignon::InternalDependency::new(
                                        "ids".to_owned(),
                                        ::sauvignon::DependencyType::ListOfIds,
                                        ::sauvignon::InternalDependencyResolver::ColumnGetterList(::sauvignon::ColumnGetterList::new(
                                            #foreign_key_table_name.to_owned(),
                                            "id".to_owned(),
                                            vec![::sauvignon::Where::new(#foreign_key.to_owned())],
                                        )),
                                    )],
                                    ::sauvignon::CarverOrPopulator::PopulatorList(::sauvignon::ValuePopulatorList::new("id".to_owned()).into())
                                ))
                                .build()
                                .unwrap()
                        }
                    }
                    (None, Some(through)) => {
                        let through_self_column_name = through_self_column_name.as_ref().unwrap();
                        let through_other_column_name = through_other_column_name.as_ref().unwrap();
                        quote! {
                            ::sauvignon::TypeFieldBuilder::default()
                                .name(#name)
                                .type_(::sauvignon::TypeFull::List(::std::boxed::Box::new(::sauvignon::TypeFull::Type(#type_.to_owned()))))
                                .resolver(::sauvignon::FieldResolver::new(
                                    vec![::sauvignon::ExternalDependency::new("id".to_owned(), ::sauvignon::DependencyType::Id)],
                                    vec![::sauvignon::InternalDependency::new(
                                        "ids".to_owned(),
                                        ::sauvignon::DependencyType::ListOfIds,
                                        ::sauvignon::InternalDependencyResolver::ColumnGetterList(::sauvignon::ColumnGetterList::new(
                                            #through.to_owned(),
                                            #through_other_column_name.to_owned(),
                                            vec![::sauvignon::Where::new(#through_self_column_name.to_owned())],
                                        )),
                                    )],
                                    ::sauvignon::CarverOrPopulator::PopulatorList(::sauvignon::ValuePopulatorList::new("id".to_owned()).into())
                                ))
                                .build()
                                .unwrap()
                        }
                    }
                    _ => unreachable!()
                }
            }
        }
        .to_tokens(tokens)
    }
}

enum FieldValue {
    StringColumn,
    Object {
        type_: TypeFull,
        internal_dependencies: Option<Vec<InternalDependency>>,
        params: Option<Vec<Param>>,
        carver_or_populator: Option<CarverOrPopulator>,
    },
    BelongsTo {
        type_: String,
        polymorphic: bool,
    },
    HasMany {
        type_: String,
        foreign_key: Option<String>,
        through: Option<String>,
    },
}

impl FieldValue {
    pub fn process(
        self,
        parent_type_name: Option<&str>,
        all_union_or_interface_type_names: &HashSet<String>,
        all_enum_names: &HashSet<String>,
    ) -> FieldValueProcessed {
        match self {
            Self::StringColumn => FieldValueProcessed::StringColumn {
                table_name: pluralize(&parent_type_name.unwrap().to_snake_case()),
            },
            Self::Object {
                type_,
                internal_dependencies,
                params,
                carver_or_populator,
            } => FieldValueProcessed::Object {
                internal_dependencies: internal_dependencies.map(|internal_dependencies| {
                    internal_dependencies
                        .into_iter()
                        .map(|internal_dependency| internal_dependency.process(type_.name()))
                        .collect()
                }),
                maybe_type_kind: if all_union_or_interface_type_names.contains(type_.name()) {
                    Some(TypeKind::UnionOrInterface)
                } else if all_enum_names.contains(type_.name()) {
                    Some(TypeKind::Enum)
                } else {
                    None
                },
                type_,
                params,
                carver_or_populator,
            },
            Self::BelongsTo { type_, polymorphic } => FieldValueProcessed::BelongsTo {
                type_,
                self_table_name: pluralize(&parent_type_name.unwrap().to_snake_case()),
                polymorphic,
            },
            Self::HasMany {
                type_,
                foreign_key,
                through,
            } => {
                let through_self_column_name = through
                    .is_some()
                    .then(|| format!("{}_id", parent_type_name.unwrap().to_snake_case()));
                let through_other_column_name = through
                    .is_some()
                    .then(|| format!("{}_id", type_.to_snake_case()));
                FieldValueProcessed::HasMany {
                    type_,
                    foreign_key,
                    through,
                    through_self_column_name,
                    through_other_column_name,
                }
            }
        }
    }
}

impl Parse for FieldValue {
    fn parse(input: ParseStream) -> Result<Self> {
        match input.parse::<Ident>() {
            Ok(ident) => match &*ident.to_string() {
                "string_column" => {
                    let arguments_content;
                    parenthesized!(arguments_content in input);
                    if !arguments_content.is_empty() {
                        return Err(arguments_content.error("Not expecting argument values"));
                    }
                    Ok(Self::StringColumn)
                }
                "belongs_to" => {
                    let arguments_content;
                    parenthesized!(arguments_content in input);
                    let mut type_: Option<String> = _d();
                    let mut polymorphic: Option<bool> = _d();
                    while !arguments_content.is_empty() {
                        let key = parse_ident_or_type(&arguments_content)?;
                        arguments_content.parse::<Token![=>]>()?;
                        match &*key.to_string() {
                            "type" => {
                                type_ = Some(arguments_content.parse::<Ident>()?.to_string());
                            }
                            "polymorphic" => {
                                polymorphic = Some(arguments_content.parse::<LitBool>()?.value);
                            }
                            key => {
                                return Err(
                                    arguments_content.error(format!("Unexpected key `{key}`"))
                                )
                            }
                        }
                        arguments_content.parse::<Option<Token![,]>>()?;
                    }
                    Ok(Self::BelongsTo {
                        type_: type_.expect("Expected `type`"),
                        polymorphic: polymorphic.unwrap_or(false),
                    })
                }
                "has_many" => {
                    let arguments_content;
                    parenthesized!(arguments_content in input);
                    let mut type_: Option<String> = _d();
                    let mut foreign_key: Option<String> = _d();
                    let mut through: Option<String> = _d();
                    while !arguments_content.is_empty() {
                        let key = parse_ident_or_type(&arguments_content)?;
                        arguments_content.parse::<Token![=>]>()?;
                        match &*key.to_string() {
                            "type" => {
                                type_ = Some(arguments_content.parse::<Ident>()?.to_string());
                            }
                            "foreign_key" => {
                                foreign_key = Some(arguments_content.parse::<Ident>()?.to_string());
                            }
                            "through" => {
                                through = Some(arguments_content.parse::<Ident>()?.to_string());
                            }
                            key => {
                                return Err(
                                    arguments_content.error(format!("Unexpected key `{key}`"))
                                )
                            }
                        }
                        arguments_content.parse::<Option<Token![,]>>()?;
                    }
                    // TODO: figure this out
                    if foreign_key.is_some() && through.is_some() {
                        return Err(arguments_content.error(
                            "Currently not supporting combination of `foreign_key` and `through`",
                        ));
                    }
                    if foreign_key.is_none() && through.is_none() {
                        return Err(arguments_content.error(
                            "Currently expecting exactly one of `foreign_key` and `through`",
                        ));
                    }
                    Ok(Self::HasMany {
                        type_: type_.expect("Expected `type`"),
                        foreign_key,
                        through,
                    })
                }
                _ => return Err(input.error("Expected known field helper eg `string_column()`")),
            },
            _ => {
                let field_value_content;
                braced!(field_value_content in input);
                let mut type_: Option<TypeFull> = _d();
                let mut internal_dependencies: Option<Vec<InternalDependency>> = _d();
                let mut params: Option<Vec<Param>> = _d();
                let mut populator: Option<CarverOrPopulator> = _d();
                let mut carver: Option<CarverOrPopulator> = _d();
                while !field_value_content.is_empty() {
                    let key = parse_ident_or_type(&field_value_content)?;
                    field_value_content.parse::<Token![=>]>()?;
                    match &*key.to_string() {
                        "type" => {
                            assert!(type_.is_none(), "Already saw 'types' key");
                            type_ = Some(field_value_content.parse()?);
                        }
                        "internal_dependencies" => {
                            assert!(
                                internal_dependencies.is_none(),
                                "Already saw 'internal_dependencies' key"
                            );
                            let internal_dependencies_content;
                            bracketed!(internal_dependencies_content in field_value_content);
                            let internal_dependencies = internal_dependencies.populate_default();
                            while !internal_dependencies_content.is_empty() {
                                internal_dependencies.push(internal_dependencies_content.parse()?);
                                internal_dependencies_content.parse::<Option<Token![,]>>()?;
                            }
                        }
                        "params" => {
                            assert!(params.is_none(), "Already saw 'params' key");
                            let params_content;
                            bracketed!(params_content in field_value_content);
                            let params = params.populate_default();
                            while !params_content.is_empty() {
                                params.push(params_content.parse()?);
                                params_content.parse::<Option<Token![,]>>()?;
                            }
                        }
                        "populator" => {
                            assert!(populator.is_none(), "Already saw 'populator' key");
                            assert!(carver.is_none(), "Already saw 'carver' key, can't have both 'carver' and 'populator'");
                            populator = Some(field_value_content.parse()?);
                        }
                        "carver" => {
                            assert!(carver.is_none(), "Already saw 'carver' key");
                            assert!(populator.is_none(), "Already saw 'populator' key, can't have both 'carver' and 'populator'");
                            carver = Some(field_value_content.parse()?);
                        }
                        key => {
                            return Err(field_value_content.error(format!("Unexpected key `{key}`")))
                        }
                    }
                    field_value_content.parse::<Option<Token![,]>>()?;
                }
                Ok(Self::Object {
                    type_: type_.expect("Expected `type`"),
                    internal_dependencies,
                    params,
                    carver_or_populator: carver.or(populator),
                })
            }
        }
    }
}

enum FieldValueProcessed {
    StringColumn {
        table_name: String,
    },
    Object {
        type_: TypeFull,
        internal_dependencies: Option<Vec<InternalDependencyProcessed>>,
        params: Option<Vec<Param>>,
        carver_or_populator: Option<CarverOrPopulator>,
        maybe_type_kind: Option<TypeKind>,
    },
    BelongsTo {
        type_: String,
        self_table_name: String,
        polymorphic: bool,
    },
    HasMany {
        type_: String,
        foreign_key: Option<String>,
        through: Option<String>,
        through_self_column_name: Option<String>,
        through_other_column_name: Option<String>,
    },
}

struct InternalDependency {
    pub name: String,
    pub type_: InternalDependencyType,
}

impl InternalDependency {
    pub fn process(self, field_type_name: &str) -> InternalDependencyProcessed {
        InternalDependencyProcessed {
            name: self.name,
            type_: self.type_.process(field_type_name),
        }
    }
}

impl Parse for InternalDependency {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = parse_ident_or_type(&input)?;
        input.parse::<Token![=>]>()?;
        let type_: InternalDependencyType = input.parse()?;
        Ok(Self {
            name: name.to_string(),
            type_,
        })
    }
}

#[derive(Clone)]
struct InternalDependencyProcessed {
    pub name: String,
    pub type_: InternalDependencyTypeProcessed,
}

impl ToTokens for InternalDependencyProcessed {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = &self.name;
        let type_ = match &self.type_ {
            InternalDependencyTypeProcessed::LiteralValue(_) => quote! {
                ::sauvignon::DependencyType::Id
            },
            InternalDependencyTypeProcessed::Param(dependency_type) => quote! {
                #dependency_type
            },
            InternalDependencyTypeProcessed::IdColumnList { .. } => quote! {
                ::sauvignon::DependencyType::ListOfIds
            },
        };
        let resolver = match &self.type_ {
            InternalDependencyTypeProcessed::LiteralValue(dependency_value) => quote! {
                ::sauvignon::InternalDependencyResolver::LiteralValue(
                    ::sauvignon::LiteralValueInternalDependencyResolver(#dependency_value)
                )
            },
            InternalDependencyTypeProcessed::IdColumnList { field_type_name } => {
                let table_name = pluralize(&field_type_name.to_snake_case());
                quote! {
                    ::sauvignon::InternalDependencyResolver::ColumnGetterList(::sauvignon::ColumnGetterList::new(
                        #table_name.to_owned(),
                        "id".to_owned(),
                        vec![],
                    ))
                }
            }
            InternalDependencyTypeProcessed::Param(_) => {
                quote! {
                    ::sauvignon::InternalDependencyResolver::Argument(
                        ::sauvignon::ArgumentInternalDependencyResolver::new(#name.to_owned()),
                    )
                }
            }
        };
        quote! {
            ::sauvignon::InternalDependency::new(
                #name.to_owned(),
                #type_,
                #resolver,
            )
        }
        .to_tokens(tokens)
    }
}

enum InternalDependencyType {
    LiteralValue(DependencyValue),
    IdColumnList { type_: Option<String> },
}

impl InternalDependencyType {
    pub fn process(self, field_type_name: &str) -> InternalDependencyTypeProcessed {
        match self {
            Self::LiteralValue(dependency_value) => {
                InternalDependencyTypeProcessed::LiteralValue(dependency_value)
            }
            Self::IdColumnList { type_ } => InternalDependencyTypeProcessed::IdColumnList {
                field_type_name: type_.unwrap_or_else(|| field_type_name.to_owned()),
            },
        }
    }
}

impl Parse for InternalDependencyType {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;
        match &*name.to_string() {
            "literal_value" => {
                let arguments_content;
                parenthesized!(arguments_content in input);
                let value: DependencyValue = arguments_content.parse()?;
                if !arguments_content.is_empty() {
                    return Err(arguments_content.error("Didn't expect more arguments"));
                }
                Ok(Self::LiteralValue(value))
            }
            "id_column_list" => {
                let arguments_content;
                parenthesized!(arguments_content in input);
                let mut type_: Option<String> = _d();
                while !arguments_content.is_empty() {
                    let key = parse_ident_or_type(&arguments_content)?;
                    arguments_content.parse::<Token![=>]>()?;
                    match &*key.to_string() {
                        "type" => {
                            assert!(type_.is_none(), "Already saw 'type' key");
                            type_ = Some(arguments_content.parse::<Ident>()?.to_string());
                        }
                        key => {
                            return Err(arguments_content.error(format!("Unexpected key `{key}`")))
                        }
                    }
                }
                Ok(Self::IdColumnList { type_ })
            }
            _ => {
                return Err(
                    input.error("Expected known internal dependency helper eg `literal_value()`")
                )
            }
        }
    }
}

#[derive(Clone)]
enum InternalDependencyTypeProcessed {
    LiteralValue(DependencyValue),
    IdColumnList { field_type_name: String },
    Param(DependencyType),
}

#[proc_macro]
pub fn schema(input: TokenStream) -> TokenStream {
    let schema: Schema = parse_macro_input!(input);
    let schema = schema.process();

    let query_field_builders = schema.query.iter().map(|query_field| {
        quote! { #query_field }
    });

    let types = schema.types.iter().map(|type_| {
        let name = &type_.name;
        let type_field_builders = type_.fields.iter().map(|field| {
            quote! { #field }
        });
        let implements = match type_.implements.as_ref() {
            None => quote! {},
            Some(implements) => {
                let implements = implements.iter().map(|implement| {
                    quote! {
                        #implement.to_owned()
                    }
                });
                quote! {
                    .implements(vec![#(#implements),*])
                }
            }
        };
        quote! {
            ::sauvignon::Type::Object(
                ::sauvignon::ObjectTypeBuilder::default()
                    .name(#name)
                    .fields([
                        #(#type_field_builders),*
                    ])
                    #implements
                    .build()
                    .unwrap()
            )
        }
    });

    let interfaces = match schema.interfaces.as_ref() {
        None => quote! { vec![] },
        Some(interfaces) => {
            let interfaces = interfaces
                .into_iter()
                .map(|interface| quote! { #interface });
            quote! { vec![#(#interfaces),*] }
        }
    };

    let unions = match schema.unions.as_ref() {
        None => quote! { vec![] },
        Some(unions) => {
            let unions = unions.into_iter().map(|union| quote! { #union });
            quote! { vec![#(#unions),*] }
        }
    };

    let enums = match schema.enums.as_ref() {
        None => quote! {},
        Some(enums) => {
            let enums = enums.into_iter().map(|enum_| quote! { #enum_ });
            quote! { #(#enums),* }
        }
    };
    quote! {{
        let query_type = ::sauvignon::Type::Object(
            ::sauvignon::ObjectTypeBuilder::default()
                .name("Query")
                .fields([
                    #(#query_field_builders),*
                ])
                .is_top_level_type(::sauvignon::OperationType::Query)
                .build()
                .unwrap()
        );

        ::sauvignon::Schema::try_new(
            vec![query_type, #(#types),*, #enums],
            #unions,
            #interfaces,
        ).unwrap()
    }}
    .into()
}

// TODO: actually share this with the sauvignon crate?
enum TypeFull {
    Type(String),
    List(Box<TypeFull>),
    NonNull(Box<TypeFull>),
}

impl TypeFull {
    pub fn name(&self) -> &str {
        match self {
            Self::Type(type_) => type_,
            Self::List(list) => list.name(),
            Self::NonNull(non_null) => non_null.name(),
        }
    }

    pub fn is_list_type(&self) -> bool {
        match self {
            Self::Type(_) => false,
            Self::List(_) => true,
            Self::NonNull(inner_type) => inner_type.is_list_type(),
        }
    }
}

impl Parse for TypeFull {
    fn parse(input: ParseStream) -> Result<Self> {
        match input.parse::<Ident>() {
            Ok(type_name) => match input.parse::<Token![!]>() {
                Ok(_) => Ok(Self::NonNull(Box::new(Self::Type(type_name.to_string())))),
                _ => Ok(Self::Type(type_name.to_string())),
            },
            _ => {
                let list_type_content;
                bracketed!(list_type_content in input);
                let inner_type: TypeFull = list_type_content.parse()?;
                if !list_type_content.is_empty() {
                    return Err(list_type_content.error("Expected only inner type"));
                }
                match input.parse::<Token![!]>() {
                    Ok(_) => Ok(Self::NonNull(Box::new(Self::List(Box::new(inner_type))))),
                    _ => Ok(Self::List(Box::new(inner_type))),
                }
            }
        }
    }
}

impl ToTokens for TypeFull {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Self::Type(type_) => quote! {
                ::sauvignon::TypeFull::Type(#type_.to_owned())
            },
            Self::List(type_) => quote! {
                ::sauvignon::TypeFull::List(::std::boxed::Box::new(#type_))
            },
            Self::NonNull(type_) => quote! {
                ::sauvignon::TypeFull::NonNull(::std::boxed::Box::new(#type_))
            },
        }
        .to_tokens(tokens)
    }
}

// TODO: possibly actually share these with the sauvignon crate?
type Id = i32;

#[derive(Clone)]
enum DependencyValue {
    Id(Id),
    String(String),
    List(Vec<DependencyValue>),
    IdentId(Ident),
}

impl Parse for DependencyValue {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(match input.peek(LitInt) {
            true => Self::Id(input.parse::<LitInt>().unwrap().base10_parse::<Id>()?),
            false => match input.peek(Ident) {
                true => match &*input.parse::<Ident>().unwrap().to_string() {
                    "id" => {
                        input.parse::<Token![=>]>()?;
                        Self::IdentId(input.parse::<Ident>()?)
                    }
                    _ => return Err(input.error("Expected `id`")),
                },
                false => Self::String(input.parse::<LitStr>()?.value()),
            },
        })
    }
}

impl ToTokens for DependencyValue {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Self::Id(id) => quote! {
                ::sauvignon::DependencyValue::Id(#id)
            },
            Self::String(string) => quote! {
                ::sauvignon::DependencyValue::String(#string.to_owned())
            },
            Self::IdentId(ident) => quote! {
                ::sauvignon::DependencyValue::Id(#ident)
            },
            _ => unimplemented!(),
        }
        .to_tokens(tokens)
    }
}

struct Interface {
    pub name: String,
    pub fields: Vec<InterfaceField>,
}

impl Parse for Interface {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse::<Ident>()?.to_string();
        input.parse::<Token![=>]>()?;
        let object_content;
        braced!(object_content in input);
        let mut fields: Option<Vec<InterfaceField>> = _d();
        while !object_content.is_empty() {
            let key: Ident = object_content.parse()?;
            object_content.parse::<Token![=>]>()?;
            match &*key.to_string() {
                "fields" => {
                    assert!(fields.is_none(), "Already saw 'fields' key");
                    let fields_content;
                    bracketed!(fields_content in object_content);
                    let fields = fields.populate_default();
                    while !fields_content.is_empty() {
                        fields.push(fields_content.parse()?);
                        fields_content.parse::<Option<Token![,]>>()?;
                    }
                }
                key => return Err(object_content.error(format!("Unexpected key `{key}`"))),
            }
            object_content.parse::<Option<Token![,]>>()?;
        }

        Ok(Self {
            name,
            fields: fields.expect("Didn't see `fields`"),
        })
    }
}

impl ToTokens for Interface {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = &self.name;
        let fields = self.fields.iter().map(|field| {
            quote! { #field }
        });
        quote! {
            ::sauvignon::InterfaceBuilder::default()
                .name(#name)
                .fields(vec![#(#fields),*])
                .build()
                .unwrap()
        }
        .to_tokens(tokens)
    }
}

struct InterfaceField {
    pub name: String,
    pub type_: TypeFull,
}

impl Parse for InterfaceField {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;
        input.parse::<Token![=>]>()?;
        let type_: TypeFull = input.parse()?;
        Ok(Self {
            name: name.to_string(),
            type_,
        })
    }
}

impl ToTokens for InterfaceField {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = &self.name;
        let type_ = &self.type_;
        quote! {
            ::sauvignon::InterfaceField::new(
                #name.to_owned(),
                #type_,
                [],
            )
        }
        .to_tokens(tokens)
    }
}

struct Param {
    pub name: String,
    pub type_: TypeFull,
}

impl Param {
    pub fn is_id(&self) -> bool {
        self.type_.name() == "Id"
    }
}

impl Parse for Param {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;
        input.parse::<Token![=>]>()?;
        let type_: TypeFull = input.parse()?;

        Ok(Self {
            name: name.to_string(),
            type_,
        })
    }
}

impl ToTokens for Param {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = &self.name;
        let type_ = &self.type_;
        quote! {
            ::sauvignon::Param::new(
                #name.to_owned(),
                #type_,
            )
        }
        .to_tokens(tokens)
    }
}

struct Union {
    pub name: String,
    pub types: Vec<String>,
}

impl Parse for Union {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;
        input.parse::<Token![=>]>()?;
        let types_content;
        bracketed!(types_content in input);
        let mut types: Vec<String> = _d();
        while !types_content.is_empty() {
            types.push(types_content.parse::<Ident>()?.to_string());
            types_content.parse::<Option<Token![,]>>()?;
        }
        Ok(Self {
            name: name.to_string(),
            types,
        })
    }
}

impl ToTokens for Union {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = &self.name;
        let types = self.types.iter().map(|type_| {
            quote! {
                #type_.to_owned()
            }
        });
        quote! {
            ::sauvignon::Union::new(
                #name.to_owned(),
                vec![#(#types),*],
            )
        }
        .to_tokens(tokens)
    }
}

enum CarverOrPopulator {
    Custom(ExprBlock),
}

impl Parse for CarverOrPopulator {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;
        if name.to_string() != "custom" {
            return Err(input.error("Expected `custom`"));
        }
        Ok(Self::Custom(input.parse()?))
    }
}

impl ToTokens for CarverOrPopulator {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Self::Custom(block) => quote! {
                #block
            }
            .to_tokens(tokens),
        }
    }
}

struct Enum {
    pub name: Ident,
    pub variants: Option<Vec<String>>,
}

impl Parse for Enum {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;
        let variants = match input.parse::<Token![=>]>() {
            Ok(_) => {
                let values_content;
                bracketed!(values_content in input);
                let mut variants: Vec<String> = _d();
                while !values_content.is_empty() {
                    variants.push(values_content.parse::<Ident>()?.to_string());
                    values_content.parse::<Option<Token![,]>>()?;
                }
                Some(variants)
            }
            _ => None,
        };
        Ok(Self { name, variants })
    }
}

impl ToTokens for Enum {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = &self.name;
        let name_str = name.to_string();
        let variants = match self.variants.as_ref() {
            None => quote! {{
                use strum::VariantNames;
                #name::VARIANTS.iter().map(|variant| (*variant).to_owned())
            }},
            Some(variants) => {
                let variants = variants
                    .iter()
                    .map(|variant| quote! { #variant.to_owned() });
                quote! { vec![#(#variants),*] }
            }
        };
        quote! {
            ::sauvignon::Type::Enum(::sauvignon::Enum::new(
                #name_str.to_owned(),
                #variants,
            ))
        }
        .to_tokens(tokens)
    }
}

enum TypeKind {
    UnionOrInterface,
    Enum,
}

// TODO: share this with sauvignon crate?
#[derive(Clone)]
enum DependencyType {
    Id,
    String,
    // ListOfIds,
    // ListOfStrings,
}

impl ToTokens for DependencyType {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Self::Id => quote! {
                ::sauvignon::DependencyType::Id
            },
            Self::String => quote! {
                ::sauvignon::DependencyType::String
            },
        }
        .to_tokens(tokens)
    }
}

fn parse_ident_or_type(input: &ParseBuffer) -> Result<Ident> {
    Ok(match input.parse::<Ident>() {
        Ok(key) => key,
        _ => {
            let key = input.parse::<Token![type]>()?;
            Ident::new("type", key.span())
        }
    })
}
