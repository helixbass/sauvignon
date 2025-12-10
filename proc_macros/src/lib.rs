use heck::ToSnakeCase;
use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use squalid::{OptionExtDefault, _d};
use syn::{
    braced, bracketed, parenthesized,
    parse::{Parse, ParseStream, Result},
    parse_macro_input,
    spanned::Spanned,
    Ident, LitInt, Token,
};

struct Schema {
    pub types: Vec<Type>,
    pub query: Vec<Field>,
    pub interfaces: Option<Vec<Interface>>,
}

impl Schema {
    pub fn process(self) -> SchemaProcessed {
        SchemaProcessed {
            types: self
                .types
                .into_iter()
                .map(|type_| type_.process())
                .collect(),
            query: self
                .query
                .into_iter()
                .map(|field| field.process(None))
                .collect(),
            interfaces: self.interfaces,
        }
    }
}

impl Parse for Schema {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut types: Option<Vec<Type>> = _d();
        let mut query: Option<Vec<Field>> = _d();
        let mut interfaces: Option<Vec<Interface>> = _d();

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
                key => return Err(input.error(format!("Unexpected key `{key}`"))),
            }
        }

        Ok(Self {
            types: types.expect("Didn't see `types`"),
            query: query.expect("Didn't see `query`"),
            interfaces,
        })
    }
}

struct SchemaProcessed {
    pub types: Vec<TypeProcessed>,
    pub query: Vec<FieldProcessed>,
    pub interfaces: Option<Vec<Interface>>,
}

struct Type {
    pub name: String,
    pub fields: Vec<Field>,
    pub implements: Option<Vec<String>>,
}

impl Type {
    pub fn process(self) -> TypeProcessed {
        TypeProcessed {
            name: self.name.clone(),
            fields: self
                .fields
                .into_iter()
                .map(|field| field.process(Some(&self.name)))
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
    pub fn process(self, parent_type_name: Option<&str>) -> FieldProcessed {
        FieldProcessed {
            name: self.name,
            value: self.value.process(parent_type_name),
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
            } => {
                let populator = match type_.is_list_type() {
                    true => quote! {
                        ::sauvignon::CarverOrPopulator::PopulatorList(::sauvignon::ValuePopulatorList::new("id".to_owned()).into())
                    },
                    false => quote! {
                        ::sauvignon::CarverOrPopulator::Populator(::sauvignon::ValuePopulator::new("id".to_owned()).into())
                    },
                };
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
                            #populator,
                        ))
                        #params
                        .build()
                        .unwrap()
                }
            }
            FieldValueProcessed::BelongsTo {
                type_,
                self_table_name,
            } => {
                let self_belongs_to_foreign_key_column_name =
                    format!("{}_id", name.to_snake_case());
                quote! {
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
        internal_dependencies: Vec<InternalDependency>,
        params: Option<Vec<Param>>,
    },
    BelongsTo {
        type_: String,
    },
}

impl FieldValue {
    pub fn process(self, parent_type_name: Option<&str>) -> FieldValueProcessed {
        match self {
            Self::StringColumn => FieldValueProcessed::StringColumn {
                table_name: pluralize(&parent_type_name.unwrap().to_snake_case()),
            },
            Self::Object {
                type_,
                internal_dependencies,
                params,
            } => FieldValueProcessed::Object {
                internal_dependencies: internal_dependencies
                    .into_iter()
                    .map(|internal_dependency| internal_dependency.process(type_.name()))
                    .collect(),
                type_,
                params,
            },
            Self::BelongsTo { type_ } => FieldValueProcessed::BelongsTo {
                type_,
                self_table_name: pluralize(&parent_type_name.unwrap().to_snake_case()),
            },
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
                    arguments_content.parse::<Token![type]>()?;
                    arguments_content.parse::<Token![=>]>()?;
                    let type_: Ident = arguments_content.parse()?;
                    Ok(Self::BelongsTo {
                        type_: type_.to_string(),
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
                while !field_value_content.is_empty() {
                    let key = match field_value_content.parse::<Ident>() {
                        Ok(key) => key,
                        _ => {
                            let key = field_value_content.parse::<Token![type]>()?;
                            Ident::new("type", key.span())
                        }
                    };
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
                        key => {
                            return Err(field_value_content.error(format!("Unexpected key `{key}`")))
                        }
                    }
                    field_value_content.parse::<Option<Token![,]>>()?;
                }
                Ok(Self::Object {
                    type_: type_.expect("Expected `type`"),
                    internal_dependencies: internal_dependencies
                        .expect("Expected `internal_dependencies`"),
                    params,
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
        internal_dependencies: Vec<InternalDependencyProcessed>,
        params: Option<Vec<Param>>,
    },
    BelongsTo {
        type_: String,
        self_table_name: String,
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
        let name: Ident = input.parse()?;
        input.parse::<Token![=>]>()?;
        let type_: InternalDependencyType = input.parse()?;
        Ok(Self {
            name: name.to_string(),
            type_,
        })
    }
}

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
                    ))
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
    IdColumnList,
}

impl InternalDependencyType {
    pub fn process(self, field_type_name: &str) -> InternalDependencyTypeProcessed {
        match self {
            Self::LiteralValue(dependency_value) => {
                InternalDependencyTypeProcessed::LiteralValue(dependency_value)
            }
            Self::IdColumnList => InternalDependencyTypeProcessed::IdColumnList {
                field_type_name: field_type_name.to_owned(),
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
                if !arguments_content.is_empty() {
                    return Err(arguments_content.error("Didn't expect more arguments"));
                }
                Ok(Self::IdColumnList)
            }
            _ => {
                return Err(
                    input.error("Expected known internal dependency helper eg `literal_value()`")
                )
            }
        }
    }
}

enum InternalDependencyTypeProcessed {
    LiteralValue(DependencyValue),
    IdColumnList { field_type_name: String },
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
            vec![query_type, #(#types),*],
            vec![],
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

enum DependencyValue {
    Id(Id),
    String(String),
    List(Vec<DependencyValue>),
}

impl Parse for DependencyValue {
    fn parse(input: ParseStream) -> Result<Self> {
        let id: LitInt = input.parse()?;
        Ok(Self::Id(id.base10_parse::<Id>()?))
    }
}

impl ToTokens for DependencyValue {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Self::Id(id) => quote! {
                ::sauvignon::DependencyValue::Id(#id)
            },
            Self::String(string) => quote! {
                ::sauvignon::DependencyValue::String(#string)
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

// TODO: share this with sauvignon crate?
fn pluralize(value: &str) -> String {
    format!("{value}s")
}
