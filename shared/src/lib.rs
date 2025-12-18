use smol_str::{format_smolstr, SmolStr};

pub fn pluralize(value: &str) -> SmolStr {
    match value {
        "Species" => "Species".into(),
        "species" => "species".into(),
        "Person" => "People".into(),
        "person" => "people".into(),
        value => format_smolstr!("{value}s"),
    }
}
