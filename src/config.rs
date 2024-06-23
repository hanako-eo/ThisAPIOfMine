use secure_string::SecureString;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ApiConfig {
    pub listen_address: String,
    pub listen_port: u16,
    pub repo_owner: String,
    pub game_repository: String,
    pub updater_repository: String,
    pub updater_filename: String,
    pub cache_lifespan: u64,
    pub github_pat: Option<SecureString>,
    pub db_host: String,
    pub db_user: String,
    pub db_password: SecureString,
    pub db_database: String,
    pub player_nickname_maxlength: usize,
    pub player_allow_non_ascii: bool,
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
            cache_lifespan: 5 * 60,
            github_pat: None,
            db_host: "localhost".to_string(),
            db_user: "api".to_string(),
            db_password: "password".into(),
            db_database: "tsom_db".to_string(),
            player_nickname_maxlength: 16,
            player_allow_non_ascii: false,
        }
    }
}
