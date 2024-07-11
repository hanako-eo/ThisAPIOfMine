use actix_web::{post, web, HttpResponse, Responder};
use uuid::Uuid;

use crate::{
    app_data::AppData,
    errors::api::RouteError,
    game_connection_token::{
        GameConnectionToken, GameConnectionTokenPrivate, GamePlayerData, GameServerAddress,
    },
};

#[post("/v1/player/test")]
async fn player_test(app_data: web::Data<AppData>) -> Result<impl Responder, RouteError> {
    let player_data = GamePlayerData::generate(Uuid::new_v4(), "SirLynix".into());

    let server_address = GameServerAddress {
        address: app_data.config.game_server_address.clone(),
        port: app_data.config.game_server_port,
    };

    let private_token = GameConnectionTokenPrivate::generate(
        app_data.config.game_api_url.clone(),
        app_data.config.game_api_token.clone(),
        player_data,
    );
    let token = GameConnectionToken::generate(
        app_data.config.connection_token_key.into(),
        app_data.config.game_api_token_duration,
        server_address,
        private_token,
    );

    Ok(HttpResponse::Ok().json(token))
}
