use actix_web::{post, web, HttpResponse, Responder};
use deadpool_postgres::tokio_postgres::types::Type;
use serde::{Deserialize, Serialize};
use token::{PlayerData, PrivateToken, ServerAddress, Token};
use uuid::Uuid;

use crate::config::ApiConfig;
use crate::errors::api::{ErrorCause, ErrorCode, RequestError, RouteError};
use crate::routes::players::validate_player_token;

mod token;

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
async fn game_connect(
    config: web::Data<ApiConfig>,
    pg_pool: web::Data<deadpool_postgres::Pool>,
    params: web::Json<GameConnectionParams>,
) -> Result<impl Responder, RouteError> {
    let pg_client = pg_pool.get().await?;
    let player_id = validate_player_token(&pg_client, &params.token).await?;

    // TODO(SirLynix): to do this with only one query
    let find_player_info = pg_client
        .prepare_typed_cached(
            "SELECT uuid, nickname FROM players WHERE id = $1",
            &[Type::INT4],
        )
        .await?;

    let player_result = pg_client
        .query_opt(&find_player_info, &[&player_id])
        .await?
        .ok_or(RouteError::InvalidRequest(RequestError::new(
            ErrorCode::AuthenticationInvalidToken,
            format!("No player has the id '{player_id}'."),
        )))?;

    let uuid: Uuid = player_result.try_get(0)?;
    let nickname: String = player_result.try_get(1)?;

    let player_data = PlayerData::generate(uuid, nickname);

    let server_address =
        ServerAddress::new(config.game_server_address.as_str(), config.game_server_port);

    let private_token = PrivateToken::generate(
        config.game_api_url.as_str(),
        config.game_api_token.as_str(),
        player_data,
    );
    let Ok(token) = Token::generate(
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
