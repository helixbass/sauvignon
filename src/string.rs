pub fn pluralize(value: &str) -> String {
    format!("{value}s")
}

pub fn singularize(value: &str) -> String {
    assert!(value.ends_with("s"));
    value[0..value.len() - 1].to_owned()
}
