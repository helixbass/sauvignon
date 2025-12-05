use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("must provide query type")]
    NoQueryTypeSpecified,
    #[error("dependency already populated: `{0}`")]
    DependencyAlreadyPopulated(String),
}

pub type Result<TSuccess> = std::result::Result<TSuccess, Error>;
