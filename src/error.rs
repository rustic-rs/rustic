//! Error types

#[cfg(feature = "rhai")]
use rhai::EvalAltResult;
use thiserror::Error;

/// Kinds of [`rhai`] errors
#[cfg(feature = "rhai")]
#[derive(Debug, Error)]
pub(crate) enum RhaiErrorKinds {
    #[error(transparent)]
    RhaiParse(#[from] rhai::ParseError),
    #[error(transparent)]
    RhaiEval(#[from] Box<EvalAltResult>),
}
