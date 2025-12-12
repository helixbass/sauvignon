pub fn pluralize(value: &str) -> String {
    match value {
        "Species" => "Species".to_owned(),
        "species" => "species".to_owned(),
        value => format!("{value}s"),
    }
}
