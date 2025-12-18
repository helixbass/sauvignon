use smol_str::SmolStr;

pub fn singularize(value: &str) -> SmolStr {
    match value {
        "species" => "species".into(),
        value => {
            assert!(value.ends_with("s"));
            value[0..value.len() - 1].into()
        }
    }
}
