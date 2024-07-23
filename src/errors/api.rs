use actix_web::body::BoxBody;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, HttpResponseBuilder, ResponseError};
use serde::{Serialize, Serializer};
use std::fmt;

#[derive(Debug)]
pub enum ErrorCause {
    Database,
    Internal,
}

#[derive(Debug, Clone)]
pub enum ErrorCode {
    FetchUpdaterRelease,
    FetchGameRelease,

    NicknameEmpty,
    NicknameToolong,
    NicknameForbiddenCharacters,

    AuthenticationInvalidToken,
    EmptyToken,
    TokenGenerationFailed,

    External(String),
    Internal,
}

#[derive(Debug, Serialize)]
pub struct RequestError {
    err_code: ErrorCode,
    err_desc: String,
}

#[derive(Debug, Serialize)]
pub struct PlatformError {
    err_desc: String,
}

#[derive(Debug)]
pub enum RouteError {
    ServerError(ErrorCause, ErrorCode),
    InvalidRequest(RequestError),
    NotFoundPlatform(PlatformError),
}

impl ErrorCode {
    pub fn as_str(&self) -> &str {
        match self {
            Self::FetchUpdaterRelease => "fetch_updater_release",
            Self::FetchGameRelease => "fetch_game_release",

            Self::NicknameEmpty => "nickname_empty",
            Self::NicknameToolong => "nickname_toolong",
            Self::NicknameForbiddenCharacters => "nickname_forbidden_characters",

            Self::AuthenticationInvalidToken => "authentication_invalid_token",
            Self::EmptyToken => "empty_token",
            Self::TokenGenerationFailed => "token_generation_failed",

            Self::External(str) => str.as_str(),
            Self::Internal => "internal",
        }
    }
}

impl Serialize for ErrorCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_str().serialize(serializer)
    }
}

impl RequestError {
    pub fn new(err_code: ErrorCode, err_desc: String) -> Self {
        Self { err_code, err_desc }
    }
}

impl PlatformError {
    pub fn new(err_desc: String) -> Self {
        Self { err_desc }
    }
}

impl fmt::Display for RouteError {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unimplemented!()
    }
}

impl ResponseError for RouteError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::ServerError(..) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            Self::NotFoundPlatform(_) => StatusCode::NOT_FOUND,
        }
    }

    fn error_response(&self) -> HttpResponse<BoxBody> {
        let mut response = HttpResponseBuilder::new(self.status_code());
        match self {
            Self::ServerError(cause, err_code) => {
                log::error!("{cause:?} error: {}", err_code.as_str());
                response.json(RequestError {
                    err_code: match err_code {
                        ErrorCode::External(_) => ErrorCode::Internal,
                        err_code => err_code.clone(),
                    },
                    err_desc: match err_code {
                        ErrorCode::External(_) | ErrorCode::Internal => {
                            "an internal error occured, please retry later.".to_string()
                        }
                        err_code => err_code.as_str().to_string(),
                    },
                })
            }
            Self::InvalidRequest(err) => {
                log::error!("{:?} error: {}", err.err_code, err.err_desc);
                response.json(err)
            },
            Self::NotFoundPlatform(err) => {
                log::error!("Platform error: {}", err.err_desc);
                response.json(err)
            },
        }
    }
}

// to delete '$into_type:path' you need to use proc macros and further manipulation of the AST
macro_rules! error_from {
    (transform $from:path, $into_type:path, |$err_name:ident| $blk:block) => {
        impl From<$from> for $into_type {
            fn from($err_name: $from) -> Self {
                $blk
            }
        }
    };
    (transform_io $from:path, $into_type:path) => {
        impl From<$from> for $into_type {
            fn from(err: $from) -> Self {
                std::io::Error::from(err).into()
            }
        }
    };
}

error_from! { transform_io rand_core::Error, RouteError }
error_from! { transform std::io::Error, RouteError, |value| {
    RouteError::ServerError(
        ErrorCause::Internal,
        ErrorCode::External(value.to_string())
    )
} }
error_from! { transform tokio_postgres::Error, RouteError, |value| {
    RouteError::ServerError(
        ErrorCause::Database,
        ErrorCode::External(value.to_string())
    )
} }

error_from! { transform deadpool_postgres::PoolError, RouteError, |value| {
    RouteError::ServerError(
        ErrorCause::Database,
        ErrorCode::External(value.to_string())
    )
} }
