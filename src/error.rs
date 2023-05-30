//! Error types

use abscissa_core::error::{BoxError, Context};
use rhai::EvalAltResult;
use std::{
    fmt::{self, Display},
    io,
    ops::Deref,
};
use thiserror::Error;

/// Kinds of errors
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum ErrorKind {
    /// Input/output error
    #[error("I/O error")]
    Io,
}

/// Kinds of [`rhai`] errors
#[derive(Debug, Error)]
pub(crate) enum RhaiErrorKinds {
    #[error(transparent)]
    RhaiParse(#[from] rhai::ParseError),
    #[error(transparent)]
    RhaiEval(#[from] Box<EvalAltResult>),
}

impl ErrorKind {
    /// Create an error context from this error
    pub(crate) fn context(self, source: impl Into<BoxError>) -> Context<Self> {
        Context::new(self, Some(source.into()))
    }
}

/// Error type
#[derive(Debug)]
pub(crate) struct Error(Box<Context<ErrorKind>>);

impl Deref for Error {
    type Target = Context<ErrorKind>;

    fn deref(&self) -> &Context<ErrorKind> {
        &self.0
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Context::new(kind, None).into()
    }
}

impl From<Context<ErrorKind>> for Error {
    fn from(context: Context<ErrorKind>) -> Self {
        Self(Box::new(context))
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        ErrorKind::Io.context(err).into()
    }
}
