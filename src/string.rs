pub fn singularize(value: &str) -> String {
    match value {
        "species" => "species".to_owned(),
        value => {
            assert!(value.ends_with("s"));
            value[0..value.len() - 1].to_owned()
        }
    }
}
