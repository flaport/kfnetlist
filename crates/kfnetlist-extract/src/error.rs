//! Domain failures preserve enough detail for each caller's error mapping.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Native(#[from] rlayout::Error),
    #[error(transparent)]
    Model(#[from] kfnetlist_core::Error),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Missing(String),
    #[error("list index out of range")]
    EmptyConnectivity,
}
pub type Result<T> = std::result::Result<T, Error>;
