use rkyv::{Archive, Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Archive, Serialize, Deserialize)]
pub enum OperationType {
    Query,
    Mutation,
    Subscription,
}
