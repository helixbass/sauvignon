use proc_macro::TokenStream;
use squalid::{OptionExtDefault, _d};
use syn::{
    bracketed,
    parse::{Parse, ParseStream, Result},
    parse_macro_input, Ident, Token,
};

struct Schema {
    pub types: Vec<Type>,
}

impl Parse for Schema {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut types: Option<Vec<Type>> = _d();

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
                        let type_: Type = types_content.parse()?;
                        types.push(type_);
                        types_content.parse::<Option<Token![,]>>()?;
                    }
                }
                key => panic!("Unexpected key `{key}`"),
            }
        }

        Ok(Self {
            types: types.expect("Didn't see `types`"),
        })
    }
}

struct Type {
    pub name: String,
}

impl Parse for Type {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;
        input.parse::<Token![=>]>()?;

        Ok(Self {
            name: name.to_string(),
        })
    }
}

#[proc_macro]
pub fn schema(input: TokenStream) -> TokenStream {
    let schema: Schema = parse_macro_input!(input);
}
