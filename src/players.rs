use actix_web::{post, web, HttpResponse, Responder};
use base64::prelude::*;
use base64::Engine;
use deadpool_postgres::tokio_postgres::types::Type;

use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app_data::AppData;
use crate::errors::api::{ErrorCode, RequestError, RouteError};

pub async fn validate_player_token(
    pg_client: &deadpool_postgres::Client,
    token: &str,
) -> Result<i32, RouteError> {
    if token.is_empty() || token.len() > 64 {
        return Err(RouteError::InvalidRequest(RequestError::new(
            ErrorCode::AuthenticationInvalidToken,
            "Invalid token".to_string(),
        )));
    }

    let find_token_statement = pg_client
        .prepare_typed_cached(
            "SELECT player_id FROM player_tokens WHERE token = $1",
            &[Type::VARCHAR],
        )
        .await?;

    let token_result = pg_client.query(&find_token_statement, &[&token]).await?;
    if token_result.is_empty() {
        return Err(RouteError::InvalidRequest(RequestError::new(
            ErrorCode::AuthenticationInvalidToken,
            "Invalid token".to_string(),
        )));
    }

    Ok(token_result[0].get(0))
}

#[derive(Deserialize)]
struct CreatePlayerParams {
    nickname: String,
}

#[derive(Serialize)]
struct CreatePlayerResponse {
    uuid: String,
    token: String,
}

#[post("/v1/players")]
async fn route_player_create(
    app_data: web::Data<AppData>,
    pg_pool: web::Data<deadpool_postgres::Pool>,
    params: web::Json<CreatePlayerParams>,
) -> Result<impl Responder, RouteError> {
    let nickname = params.nickname.trim();

    if nickname.is_empty() {
        return Err(RouteError::InvalidRequest(RequestError::new(
            ErrorCode::NicknameEmpty,
            "Nickname cannot be empty".to_string(),
        )));
    }

    if nickname.len() > app_data.config.player_nickname_maxlength {
        return Err(RouteError::InvalidRequest(RequestError::new(
            ErrorCode::NicknameToolong,
            format!(
                "Nickname size exceeds maximum size of {}",
                app_data.config.player_nickname_maxlength
            ),
        )));
    }

    if !app_data.config.player_allow_non_ascii
        && !nickname
            .chars()
            .all(|x| x.is_ascii_alphanumeric() || x == ' ' || x == '_')
    {
        return Err(RouteError::InvalidRequest(RequestError::new(
            ErrorCode::NicknameForbiddenCharacters,
            "Nickname can only have ascii characters".to_string(),
        )));
    }

    let uuid = Uuid::new_v4();

    let mut pg_client = pg_pool.get().await?;

    let create_player_statement = pg_client
        .prepare_typed_cached(
            "INSERT INTO players(uuid, creation_time, nickname) VALUES($1, NOW(), $2) RETURNING id",
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

    let token = BASE64_STANDARD.encode(key);

    let transaction = pg_client.transaction().await?;
    let result = transaction
        .query(&create_player_statement, &[&uuid, &nickname])
        .await?;
    let player_id: i32 = result[0].get(0);
    transaction
        .query(&create_token_statement, &[&token, &player_id])
        .await?;
    transaction.commit().await?;

    pg_client
        .query(&create_player_statement, &[&uuid, &nickname])
        .await?;

    Ok(HttpResponse::Ok().json(CreatePlayerResponse {
        uuid: uuid.to_string(),
        token: token.to_string(),
    }))
}

#[derive(Deserialize)]
struct AuthenticationParams {
    token: String,
}

#[derive(Serialize)]
struct AuthenticationResponse {
    uuid: String,
    nickname: String,
}

#[post("/v1/player/auth")]
async fn route_player_auth(
    pg_pool: web::Data<deadpool_postgres::Pool>,
    params: web::Json<AuthenticationParams>,
) -> Result<impl Responder, RouteError> {
    let pg_client = pg_pool.get().await?;
    let player_id = validate_player_token(&pg_client, &params.token).await?;

    let find_player_info = pg_client
        .prepare_typed_cached(
            "SELECT uuid, nickname FROM players WHERE id = $1",
            &[Type::INT4],
        )
        .await?;

    let player_result = pg_client.query(&find_player_info, &[&player_id]).await?;
    if player_result.is_empty() {
        return Err(RouteError::InvalidRequest(RequestError::new(
            ErrorCode::AuthenticationInvalidToken,
            "Invalid token".to_string(),
        )));
    }

    // Update last connection time in a separate task as its result won't affect the route
    tokio::spawn(async move {
        match pg_client
            .prepare_typed_cached(
                "UPDATE players SET last_connection_time = NOW() WHERE id = $1",
                &[Type::INT4],
            )
            .await
        {
            Ok(statement) => {
                let res = pg_client.query(&statement, &[&player_id]).await;
                if let Err(err) = res {
                    eprintln!("failed to update player {player_id} connection time: {err}");
                }
            }
            Err(err) => {
                eprintln!("failed to update player {player_id} connection time (failed to prepare query): {err}");
            }
        }
    });

    let uuid: Uuid = player_result[0].get(0);
    let nickname: String = player_result[0].get(1);

    Ok(HttpResponse::Ok().json(AuthenticationResponse {
        uuid: uuid.to_string(),
        nickname,
    }))
}
