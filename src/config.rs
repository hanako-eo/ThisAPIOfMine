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
        }
    }
}
