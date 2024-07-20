use actix_web::{post, web, HttpResponse, Responder};
use deadpool_postgres::tokio_postgres::types::Type;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::ApiConfig;
use crate::errors::api::{ErrorCause, ErrorCode, RequestError, RouteError};
use crate::game_connection_token::{
    GameConnectionToken, GameConnectionTokenPrivate, GamePlayerData, GameServerAddress,
};
use crate::players::validate_player_token;

#[derive(Deserialize)]
struct GameConnectionParams {
    token: String,
}

#[derive(Serialize)]
struct GameConnectionResponse {
    uuid: String,
    nickname: String,
}

#[post("/v1/game/connect")]
async fn route_game_connect(
    config: web::Data<ApiConfig>,
    pg_pool: web::Data<deadpool_postgres::Pool>,
    params: web::Json<GameConnectionParams>,
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

    let uuid: Uuid = player_result[0].get(0);
    let nickname: String = player_result[0].get(1);

    let player_data = GamePlayerData::generate(uuid, nickname);

    let server_address = GameServerAddress {
        address: config.game_server_address.clone(),
        port: config.game_server_port,
    };

    let private_token = GameConnectionTokenPrivate::generate(
        config.game_api_url.clone(),
        config.game_api_token.clone(),
        player_data,
    );
    let Ok(token) = GameConnectionToken::generate(
        config.connection_token_key.into(),
        config.game_api_token_duration,
        server_address,
        private_token,
    ) else {
        return Err(RouteError::ServerError(
            ErrorCause::Internal,
            ErrorCode::TokenGenerationFailed,
        ));
    };

    Ok(HttpResponse::Ok().json(token))
}
