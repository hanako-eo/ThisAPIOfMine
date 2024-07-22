use actix_web::body::BoxBody;
use actix_web::http::StatusCode;
use actix_web::{HttpResponse, ResponseError};
use serde::{Serialize, Serializer};
use std::fmt;
use strum::AsRefStr;

#[derive(Debug)]
pub enum ErrorCause {
    Database,
    Internal,
}

#[derive(Debug, Clone, AsRefStr)]
#[strum(serialize_all = "snake_case")]
pub enum ErrorCode {
    FetchUpdaterRelease,
    FetchGameRelease,

    NicknameEmpty,
    NicknameToolong,
    NicknameForbiddenCharacters,

    AuthenticationInvalidToken,
    EmptyToken,
    TokenGenerationFailed,

    #[strum(to_string = "{0}")]
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

impl Serialize for ErrorCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_ref().serialize(serializer)
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
        match self {
            Self::ServerError(cause, err_code) => {
                eprintln!("{cause:?} error: {}", err_code.as_ref());
                HttpResponse::InternalServerError().json(RequestError {
                    err_code: match err_code {
                        ErrorCode::External(_) => ErrorCode::Internal,
                        err_code => err_code.clone(),
                    },
                    err_desc: match err_code {
                        ErrorCode::External(_) | ErrorCode::Internal => {
                            "an internal error occured, please retry later.".to_string()
                        }
                        err_code => err_code.as_ref().to_string(),
                    },
                })
            }
            Self::InvalidRequest(err) => HttpResponse::BadRequest().json(err),
            Self::NotFoundPlatform(err) => HttpResponse::NotFound().json(err),
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
