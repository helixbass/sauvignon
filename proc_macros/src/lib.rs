use heck::ToPascalCase;
use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use squalid::{OptionExtDefault, _d};
use syn::{
    braced, bracketed, parenthesized,
    parse::{Parse, ParseStream, Result},
    parse_macro_input, Ident, LitInt, Token,
};

struct Schema {
    pub types: Vec<Type>,
    pub query: Vec<Field>,
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
        }
    }
}

impl Parse for Schema {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut types: Option<Vec<Type>> = _d();
        let mut query: Option<Vec<Field>> = _d();

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
                key => panic!("Unexpected key `{key}`"),
            }
        }

        Ok(Self {
            types: types.expect("Didn't see `types`"),
            query: query.expect("Didn't see `query`"),
        })
    }
}

struct SchemaProcessed {
    pub types: Vec<TypeProcessed>,
    pub query: Vec<FieldProcessed>,
}

struct Type {
    pub name: String,
    pub fields: Vec<Field>,
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
                key => panic!("Unexpected key `{key}`"),
            }
            type_content.parse::<Option<Token![,]>>()?;
        }

        Ok(Self {
            name: name.to_string(),
            fields: fields.expect("Didn't see `fields`"),
        })
    }
}

struct TypeProcessed {
    pub name: String,
    pub fields: Vec<FieldProcessed>,
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
        match &self.value {
            FieldValueProcessed::StringColumn {
                table_name,
            } => {
                quote! {
                    ::sauvignon::TypeFieldBuilder::default()
                        .name(#{self.name})
                        .type_(::sauvignon::TypeFull::Type("String".to_owned()))
                        .resolver(::sauvignon::FieldResolver::new(
                            vec![::sauvignon::ExternalDependency::new("id".to_owned(), ::sauvignon::DependencyType::Id)],
                            vec![::sauvignon::InternalDependency::new(
                                #{self.name}.to_owned(),
                                ::sauvignon::DependencyType::String,
                                ::sauvignon::InternalDependencyResolver::ColumnGetter(::sauvignon::ColumnGetter::new(
                                    #table_name.to_owned(),
                                    #{self.name}.to_owned(),
                                )),
                            )],
                            ::sauvignon::CarverOrPopulator::Carver(Box::new(::sauvignon::StringCarver::new(#{self.name}.to_owned()))),
                        ))
                        .build()
                        .unwrap(),
                }
            }
            FieldValueProcessed::Object {
                type_,
                internal_dependencies,
            } => {
                quote! {
                    ::sauvignon::TypeFieldBuilder::default()
                        .name(#self.name)
                        .
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
    },
}

impl FieldValue {
    pub fn process(self, parent_type_name: Option<&str>) -> FieldValueProcessed {
        match self {
            Self::StringColumn => FieldValueProcessed::StringColumn {
                table_name: pluralize(&parent_type_name.unwrap().to_pascal_case()),
            },
            Self::Object {
                type_,
                internal_dependencies,
            } => FieldValueProcessed::Object {
                type_,
                internal_dependencies,
            },
        }
    }
}

impl Parse for FieldValue {
    fn parse(input: ParseStream) -> Result<Self> {
        match input.parse::<Ident>() {
            Ok(ident) => {
                if ident.to_string() != "string_column" {
                    panic!("Expected `string_column`");
                }
                let arguments_content;
                parenthesized!(arguments_content in input);
                if !arguments_content.is_empty() {
                    panic!("Not expecting argument values");
                }
                Ok(Self::StringColumn)
            }
            _ => {
                let field_value_content;
                braced!(field_value_content in input);
                let mut type_: Option<TypeFull> = _d();
                let mut internal_dependencies: Option<Vec<InternalDependency>> = _d();
                let key: Ident = field_value_content.parse()?;
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
                    key => panic!("Unexpected key `{key}`"),
                }
                Ok(Self::Object {
                    type_: type_.expect("Expected `type`"),
                    internal_dependencies: internal_dependencies
                        .expect("Expected `internal_dependencies`"),
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
        internal_dependencies: Vec<InternalDependency>,
    },
}

struct InternalDependency {
    pub name: String,
    pub type_: InternalDependencyType,
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

enum InternalDependencyType {
    LiteralValue(DependencyValue),
}

impl Parse for InternalDependencyType {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;
        if name.to_string() != "literal_value" {
            panic!("Expected `literal_value`");
        }
        let arguments_content;
        parenthesized!(arguments_content in input);
        let value: DependencyValue = arguments_content.parse()?;
        if !arguments_content.is_empty() {
            panic!("Didn't expect more arguments");
        }
        Ok(Self::LiteralValue(value))
    }
}

#[proc_macro]
pub fn schema(input: TokenStream) -> TokenStream {
    let schema: Schema = parse_macro_input!(input);
    let schema = schema.process();

    let query_field_builders = schema.query.iter().map(|query_field| {
        quote! { #query_field }
    });

    quote! {
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
            vec![query_type, ],
            vec![],
            vec![],
        )?
    }
    .into()
}

// TODO: actually share this with the sauvignon crate?
enum TypeFull {
    Type(String),
    List(Box<TypeFull>),
    NonNull(Box<TypeFull>),
}

impl Parse for TypeFull {
    fn parse(input: ParseStream) -> Result<Self> {
        let type_name: Ident = input.parse()?;
        match input.parse::<Token![!]>() {
            Ok(_) => Ok(Self::NonNull(Box::new(Self::Type(type_name.to_string())))),
            _ => Ok(Self::Type(type_name.to_string())),
        }
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

// TODO: share this with sauvignon crate?
fn pluralize(value: &str) -> String {
    format!("{value}s")
}
