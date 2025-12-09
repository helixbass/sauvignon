use sauvignon::schema;

#[test]
fn test_column_getter() {
    let schema = schema! {
        types => [
            Actor => {
                fields => [
                    name => string_column(),
                ]
            }
        ]
        query => [
            actorKatie => {
                type => Actor!
                internal_dependencies => [
                    id => literal_value(1),
                ]
            }
        ]
    };
}
