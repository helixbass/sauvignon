#[derive(Copy, Clone, Eq, PartialEq)]
pub enum OperationType {
    Query,
    Mutation,
    Subscription,
}
