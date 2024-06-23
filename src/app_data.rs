use cached::TimedCache;
use std::sync::Mutex;

use crate::config::ApiConfig;
use crate::fetcher::Fetcher;
use crate::CachedReleased;

pub struct AppData {
    pub cache: Mutex<TimedCache<&'static str, CachedReleased>>,
    pub config: ApiConfig,
    pub fetcher: Fetcher,
}
