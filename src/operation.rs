#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum OperationType {
    Query,
    Mutation,
    Subscription,
}
