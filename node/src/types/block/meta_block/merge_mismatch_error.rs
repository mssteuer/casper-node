use thiserror::Error;
use tracing::error;

#[derive(Clone, Copy, Error, Debug)]
pub(crate) enum MergeMismatchError {
    #[error("block mismatch when merging meta blocks")]
    Block,
    #[error("execution results mismatch when merging meta blocks")]
    ExecutionResults,
}
