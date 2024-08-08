#[cfg(test)]
use mockall::automock;

use crate::errors::{InternalError, Result};
use crate::game_data::Asset;

#[cfg_attr(test, automock)]
pub trait ChecksumFetcher {
    async fn resolve_asset(&self, asset: &Asset) -> Result<String>;
}

pub struct HttpChecksumFetcher(reqwest::Client);

impl HttpChecksumFetcher {
    pub fn new() -> Self {
        Self(reqwest::Client::new())
    }

    fn parse_response(&self, asset_name: &str, response: &str) -> Result<String> {
        let parts: Vec<_> = response.split_whitespace().collect();
        if parts.len() != 2 {
            return Err(InternalError::InvalidSha256(parts.len()));
        }

        let (sha256, filename) = (parts[0], parts[1]);
        match !filename.starts_with('*') || &filename[1..] != asset_name {
            false => Ok(sha256.to_string()),
            true => Err(InternalError::WrongChecksum),
        }
    }
}

impl ChecksumFetcher for HttpChecksumFetcher {
    async fn resolve_asset(&self, asset: &Asset) -> Result<String> {
        let response = self
            .0
            .get(format!("{}.sha256", asset.download_url))
            .send()
            .await?
            .text()
            .await?;

        self.parse_response(asset.name.as_str(), response.as_str())
    }
}
