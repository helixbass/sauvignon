use proc_macro::TokenStream;
use squalid::{OptionExtDefault, _d};
use syn::{
    braced, bracketed, parenthesized,
    parse::{Parse, ParseStream, Result},
    parse_macro_input, Ident, Token,
};

struct Schema {
    pub types: Vec<Type>,
    pub query: Vec<Field>,
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
                    unimplemented!()
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

struct Type {
    pub name: String,
    pub fields: Vec<Field>,
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

struct Field {
    pub name: String,
    pub value: FieldValue,
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

enum FieldValue {
    StringColumn,
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
                unimplemented!()
            }
        }
    }
}

#[proc_macro]
pub fn schema(input: TokenStream) -> TokenStream {
    let schema: Schema = parse_macro_input!(input);
}
