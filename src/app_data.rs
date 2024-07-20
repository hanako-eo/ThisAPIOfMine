use cached::TimedCache;
use tokio::sync::Mutex;

use crate::fetcher::Fetcher;
use crate::routes::version::CachedReleased;

pub struct AppData {
    pub cache: Mutex<TimedCache<&'static str, CachedReleased>>,
    pub fetcher: Fetcher,
}
