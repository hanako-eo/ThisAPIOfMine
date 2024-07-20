pub mod api;

use std::error::Error;

use crate::metaprog::type_eq;

pub type Result<T, E = InternalError> = std::result::Result<T, E>;

#[derive(Debug)]
pub enum InternalError {
    // FetcherError
    InvalidSha256(usize),
    WrongChecksum,
    NoReleaseFound,
    InvalidVersion,

    // ConnectionTokenError
    SystemTimeError,

    External(Box<dyn Error + Send>),
}

impl InternalError {
    #[inline]
    pub fn is<T: Error + 'static>(&self) -> bool {
        match self {
            Self::External(err) => err.is::<T>(),
            _ => false,
        }
    }
}

impl<E: Error + Send + 'static> From<E> for InternalError {
    #[inline]
    fn from(value: E) -> Self {
        // we need these conditions because unfortunately with impl if we do:
        // ```rs
        // impl From<std::time::SystemTimeError> for InternalError {
        //     fn from(_: std::time::SystemTimeError) -> Self {
        //         InternalError::SystemTimeError
        //     }
        // }
        // impl<E: Error + Send + 'static> From<E> for InternalError {
        //     fn from(value: E) -> Self {
        //         InternalError::External(Box::new(value))
        //     }
        // }
        // ```
        // rust gives us the error: ``error[E0119]: conflicting implementations of trait `std::convert::From<SystemTimeError>` for type `errors::InternalError```

        if type_eq::<E, std::time::SystemTimeError>() {
            InternalError::SystemTimeError
        } else if type_eq::<E, semver::Error>() {
            InternalError::InvalidVersion
        } else {
            InternalError::External(Box::new(value))
        }
    }
}
