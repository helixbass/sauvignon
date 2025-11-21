use crate::{
    request, types, IndexMap, OperationType, Request, Schema, Selection, SelectionSet, Type,
};

pub struct QueryPlan<'a> {
    field_plans: Vec<FieldPlan<'a>>,
}

fn create_field_plans<'a>(
    selection_set: &'a SelectionSet,
    type_fields: &'a IndexMap<String, types::Field>,
    schema: &'a Schema,
) -> Vec<FieldPlan<'a>> {
    selection_set
        .selections
        .iter()
        .map(|selection| {
            let field = match selection {
                Selection::Field(field) => field,
                _ => panic!(),
            };

            let field_type = &type_fields[&field.name];
            FieldPlan::new(field, field_type, schema)
        })
        .collect()
}

impl<'a> QueryPlan<'a> {
    pub fn new(request: &'a Request, schema: &'a Schema) -> Self {
        let chosen_operation = request.chosen_operation();
        assert_eq!(chosen_operation.operation_type, OperationType::Query);

        let query_type = schema.query_type();
        let current_type_fields = match query_type {
            Type::Object(query_type) => &query_type.fields,
            _ => panic!(),
        };

        let field_plans =
            create_field_plans(&chosen_operation.selection_set, current_type_fields, schema);

        Self { field_plans }
    }
}

pub struct FieldPlan<'a> {
    request_field: &'a request::Field,
    field_type: &'a types::Field,
    selection_set: Option<Vec<FieldPlan<'a>>>,
}

impl<'a> FieldPlan<'a> {
    pub fn new(
        request_field: &'a request::Field,
        field_type: &'a types::Field,
        schema: &'a Schema,
        // selection_set: Vec<FieldPlan<'a>>,
    ) -> Self {
        let type_ = schema.get_type(field_type.type_.name());

        Self {
            request_field,
            field_type,
            selection_set: request_field.selection_set.as_ref().map(|selection_set| {
                create_field_plans(
                    selection_set,
                    match type_ {
                        Type::Object(type_) => &type_.fields,
                        _ => panic!(),
                    },
                    schema,
                )
            }),
        }
    }
}
