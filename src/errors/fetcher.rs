use crate::error_from;

pub type FetchResult<T> = std::result::Result<T, FetcherError>;

#[derive(Debug)]
pub enum FetcherError {
    OctoError(octocrab::Error),
    ReqwestError(reqwest::Error),
    InvalidSha256(usize),
    WrongChecksum,
    NoReleaseFound,
    InvalidVersion,
}

error_from! { move octocrab::Error, FetcherError, FetcherError::OctoError }
error_from! { move reqwest::Error, FetcherError, FetcherError::ReqwestError }
error_from! { replace semver::Error, FetcherError, FetcherError::InvalidVersion }
