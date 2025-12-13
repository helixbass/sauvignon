pub fn pluralize(value: &str) -> String {
    match value {
        "Species" => "Species".to_owned(),
        "species" => "species".to_owned(),
        "Person" => "People".to_owned(),
        "person" => "people".to_owned(),
        value => format!("{value}s"),
    }
}
