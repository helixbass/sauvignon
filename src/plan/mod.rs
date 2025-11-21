use crate::{
    fields_in_progress_new, request, types, OperationType, Request, ResponseInProgress, Schema,
    Selection, SelectionSet, Type,
};

pub struct QueryPlan<'a> {
    field_plans: Vec<FieldPlan<'a>>,
}

impl<'a> QueryPlan<'a> {
    pub fn new(request: &'a Request, schema: &'a Schema) -> Self {
        let chosen_operation = request.chosen_operation();
        assert_eq!(chosen_operation.operation_type, OperationType::Query);

        Self {
            field_plans: create_field_plans(
                &chosen_operation.selection_set,
                schema.query_type(),
                schema,
            ),
        }
    }

    pub fn initial_response_in_progress(&self) -> ResponseInProgress<'_> {
        ResponseInProgress::new(fields_in_progress_new(&self.field_plans))
    }
}

pub struct FieldPlan<'a> {
    pub request_field: &'a request::Field,
    pub field_type: &'a types::Field,
    pub selection_set: Option<Vec<FieldPlan<'a>>>,
}

impl<'a> FieldPlan<'a> {
    pub fn new(
        request_field: &'a request::Field,
        field_type: &'a types::Field,
        schema: &'a Schema,
    ) -> Self {
        Self {
            request_field,
            field_type,
            selection_set: request_field.selection_set.as_ref().map(|selection_set| {
                create_field_plans(
                    selection_set,
                    schema.get_type(field_type.type_.name()),
                    schema,
                )
            }),
        }
    }
}

fn create_field_plans<'a>(
    selection_set: &'a SelectionSet,
    type_: &'a Type,
    schema: &'a Schema,
) -> Vec<FieldPlan<'a>> {
    let type_fields = match type_ {
        Type::Object(type_) => &type_.fields,
        _ => unreachable!(),
    };

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
