use std::fmt::{self, Display};

use actix_web::body::BoxBody;
use actix_web::http::StatusCode;
use actix_web::{error, post, web, HttpResponse, Responder};
use base64::prelude::*;
use base64::Engine;
use deadpool_postgres::tokio_postgres::types::{Type};

use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app_data::AppData;


#[derive(Debug, Serialize)]
struct RequestError {
    err_code: String,
    err_desc: String,
}

#[derive(Debug)]
struct InternalError {
    err_code: String,
}

#[derive(Debug)]
enum RouteError {
    DatabaseError(InternalError),
    Internal(InternalError),
    InvalidRequest(RequestError),
}

impl Display for RouteError {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unimplemented!()
    }
}

impl From<std::io::Error> for RouteError {
    fn from(value: std::io::Error) -> Self {
        RouteError::Internal(InternalError {
            err_code: value.to_string(),
        })
    }
}

impl From<rand_core::Error> for RouteError {
    fn from(value: rand_core::Error) -> Self {
        let std_err: std::io::Error = value.into();
        std_err.into()
    }
}

impl From<tokio_postgres::Error> for RouteError {
    fn from(value: tokio_postgres::Error) -> Self {
        RouteError::DatabaseError(InternalError {
            err_code: value.to_string(),
        })
    }
}

impl error::ResponseError for RouteError {
    fn status_code(&self) -> StatusCode {
        match *self {
            RouteError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            RouteError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            RouteError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
        }
    }

    fn error_response(&self) -> HttpResponse<BoxBody> {
        match self {
            RouteError::DatabaseError(err) => {
                eprintln!(
                    "database error: {}",
                    err.err_code
                );
                HttpResponse::InternalServerError().finish()
            },
            RouteError::Internal(err) => {
                eprintln!(
                    "internal error: {}",
                    err.err_code
                );
                HttpResponse::InternalServerError().finish()
            },
            RouteError::InvalidRequest(err) => HttpResponse::BadRequest().json(err),
        }
    }
}

#[derive(Deserialize)]
struct CreatePlayerParams {
    nickname: String,
}

#[derive(Serialize)]
struct CreatePlayerResponse {
    guid: String,
    token: String,
}

#[post("/players")]
async fn create_player(
    app_data: web::Data<AppData>,
    pg_pool: web::Data<deadpool_postgres::Pool>,
    params: web::Json<CreatePlayerParams>,
) -> Result<impl Responder, RouteError> {
    let nickname = params.nickname.trim();

    if nickname.is_empty() {
        return Err(RouteError::InvalidRequest(RequestError {
            err_code: "nickname_empty".to_string(),
            err_desc: "Nickname cannot be empty".to_string(),
        }));
    }

    if nickname.len() > app_data.config.player_nickname_maxlength {
        return Err(RouteError::InvalidRequest(RequestError {
            err_code: "nickname_toolong".to_string(),
            err_desc: format!(
                "Nickname size exceeds maximum size of {}",
                app_data.config.player_nickname_maxlength
            ),
        }));
    }

    if !app_data.config.player_allow_non_ascii
        && !nickname
            .chars()
            .all(|x| x.is_ascii_alphanumeric() || x == ' ' || x == '_')
    {
        return Err(RouteError::InvalidRequest(RequestError {
            err_code: "nickname_forbidden_characters".to_string(),
            err_desc: format!("Nickname can only have ascii characters"),
        }));
    }

    let guid = Uuid::new_v4();

    let mut pg_client = pg_pool.get().await.unwrap();

    let create_player_statement = pg_client
        .prepare_typed_cached(
            "INSERT INTO players(guid, creation_time, nickname) VALUES($1, NOW(), $2) RETURNING id",
            &[Type::UUID, Type::VARCHAR],
        )
        .await?;
    let create_token_statement = pg_client
        .prepare_typed_cached(
            "INSERT INTO player_tokens(token, player_id) VALUES($1, $2)",
            &[Type::VARCHAR, Type::INT4],
        )
        .await?;

    let mut key = [0u8; 32];
    OsRng.try_fill_bytes(&mut key)?;

    let token = BASE64_STANDARD.encode(&key);

    let transaction = pg_client.transaction().await?;
    let result = transaction
        .query(&create_player_statement, &[&guid, &nickname])
        .await?;
    let player_id: i32 = result[0].get(0);
    transaction
        .query(&create_token_statement, &[&token, &player_id])
        .await?;
    transaction.commit().await?;

    pg_client
        .query(&create_player_statement, &[&guid, &nickname])
        .await?;

    Ok(HttpResponse::Ok().json(CreatePlayerResponse {
        guid: guid.to_string(),
        token: token.to_string(),
    }))
}
