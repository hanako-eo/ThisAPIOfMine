use std::time::Duration;

use secure_string::SecureString;
use serde::{Deserialize, Serialize};
use serde_with::base64::Base64;
use serde_with::serde_as;
use serde_with::DurationSeconds;

#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct ApiConfig {
    pub listen_address: String,
    pub listen_port: u16,
    pub repo_owner: String,
    pub game_repository: String,
    pub updater_repository: String,
    pub updater_filename: String,
    #[serde_as(as = "DurationSeconds<u64>")]
    pub cache_lifespan: Duration,
    pub github_pat: Option<SecureString>,
    pub db_host: String,
    pub db_user: String,
    pub db_password: SecureString,
    pub db_database: String,
    pub player_nickname_maxlength: usize,
    pub player_allow_non_ascii: bool,
    pub game_api_token: String,
    pub game_api_url: String,
    pub game_server_address: String,
    pub game_server_port: u16,
    #[serde_as(as = "DurationSeconds<u64>")]
    pub game_api_token_duration: Duration,
    #[serde_as(as = "Base64")]
    pub connection_token_key: [u8; 32],
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            listen_address: "0.0.0.0".to_string(),
            listen_port: 14770,
            repo_owner: "DigitalpulseSoftware".to_string(),
            game_repository: "ThisSpaceOfMine".to_string(),
            updater_filename: "this_updater_of_mine".to_string(),
            updater_repository: "ThisUpdaterOfMine".to_string(),
            cache_lifespan: Duration::from_secs(5 * 60),
            github_pat: None,
            db_host: "localhost".to_string(),
            db_user: "api".to_string(),
            db_password: "password".into(),
            db_database: "tsom_db".to_string(),
            player_nickname_maxlength: 16,
            player_allow_non_ascii: false,
            game_api_token: "".to_string(),
            game_api_url: "http://localhost".to_string(),
            game_server_address: "localhost".to_string(),
            game_server_port: 29536,
            game_api_token_duration: Duration::from_secs(5 * 60),
            connection_token_key: std::array::from_fn(|i| i as u8), // <=> [0, 1, .., 31]
        }
    }
}
