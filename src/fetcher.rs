use std::collections::HashMap;

use futures::future::join_all;
use octocrab::repos::RepoHandler;
use octocrab::{Octocrab, OctocrabBuilder};
use semver::Version;

use crate::config::ApiConfig;
use crate::game_data::{Asset, GameRelease, Repo};

type Result<T> = std::result::Result<T, FetcherError>;

pub struct Fetcher {
    octocrab: Octocrab,
    game_repo: Repo,
    updater_repo: Repo,

    checksum_fetcher: ChecksumFetcher,
}

struct ChecksumFetcher(reqwest::Client);

#[derive(Debug)]
pub enum FetcherError {
    OctoError(octocrab::Error),
    ReqwestError(reqwest::Error),
    InvalidSha256(usize),
    WrongChecksum,
    NoUpdaterReleaseFound,
}

impl Fetcher {
    pub fn from_config(config: &ApiConfig) -> Result<Self> {
        let mut octocrab = OctocrabBuilder::default();
        if let Some(github_pat) = &config.github_pat {
            octocrab = octocrab.personal_token(github_pat.unsecure().to_string());
        }

        Ok(Self {
            octocrab: octocrab.build()?,
            game_repo: Repo::new(&config.repo_owner, &config.game_repository),
            updater_repo: Repo::new(&config.repo_owner, &config.updater_repository),

            checksum_fetcher: ChecksumFetcher::new(),
        })
    }

    fn on_game_repo(&self) -> RepoHandler<'_> {
        self.octocrab
            .repos(self.game_repo.owner(), self.game_repo.repository())
    }

    fn on_updater_repo(&self) -> RepoHandler<'_> {
        self.octocrab
            .repos(self.updater_repo.owner(), self.updater_repo.repository())
    }

    pub async fn get_game_releases(&self) -> Result<Vec<GameRelease>> {
        let releases = self.on_game_repo().releases().list().send().await?;
        let versions_released = releases
            .into_iter()
            .rev()
            .filter(|r| !r.prerelease)
            .filter_map(|r| Version::parse(&r.tag_name).ok().map(|v| (v, r)));

        let mut game_releases = Vec::new();
        let mut latest_assets = None;

        for (version, release) in versions_released {
            let assets = release
                .assets
                .iter()
                .filter_map(|asset| {
                    match asset.name.ends_with(".sha256") {
                        false => Some(Asset::from(asset)),
                        // sha256 files ignored, they will be searched by the ChecksumFetcher
                        true => None,
                    }
                })
                .collect::<Vec<Asset>>();

            let checksums = join_all(
                assets
                    .iter()
                    .map(|asset| self.checksum_fetcher.resolve(asset)),
            )
            .await;

            let mut binaries = HashMap::new();

            let assets_and_checksums = assets.into_iter().zip(checksums);

            for (mut asset, checksum) in assets_and_checksums {
                asset.checksum = match checksum {
                    Ok(checksum) => Some(checksum),
                    Err(FetcherError::ReqwestError(_)) => None,
                    Err(err) => return Err(err),
                };

                let platform = remove_game_suffix(&asset.name);
                if platform == "assets" {
                    latest_assets = Some((version.clone(), asset));
                } else {
                    binaries.insert(platform.to_string(), asset);
                }
            }

            let Some((assets_version, assets)) = latest_assets.as_ref() else {
                // if we don't have assets URL/versions yet we must skip
                eprintln!("ignoring release {version} because no assets was found");
                continue;
            };

            game_releases.push(GameRelease {
                assets: assets.clone(),
                assets_version: assets_version.clone(),
                binaries,
                version,
            });
        }

        Ok(game_releases)
    }

    pub async fn get_updater_releases(&self) -> Result<HashMap<String, Asset>> {
        let releases = self.on_updater_repo().releases().list().send().await?;

        let Some(last_release) = releases.into_iter().find(|release| !release.prerelease) else {
            return Err(FetcherError::NoUpdaterReleaseFound);
        };

        let assets = last_release
            .assets
            .iter()
            .filter_map(|asset| {
                match asset.name.ends_with(".sha256") {
                    false => Some(Asset::from(asset)),
                    // sha256 files ignored, they will be searched by the ChecksumFetcher
                    true => None,
                }
            })
            .collect::<Vec<Asset>>();

        let checksums = join_all(
            assets
                .iter()
                .map(|asset| self.checksum_fetcher.resolve(asset)),
        )
        .await;

        let mut binaries = HashMap::new();

        let assets_and_checksums = assets.into_iter().zip(checksums);
        for (mut asset, checksum) in assets_and_checksums {
            asset.checksum = match checksum {
                Ok(checksum) => Some(checksum),
                Err(FetcherError::ReqwestError(_)) => None,
                Err(err) => return Err(err),
            };

            binaries.insert(remove_game_suffix(asset.name.as_str()).to_string(), asset);
        }

        Ok(binaries)
    }
}

impl ChecksumFetcher {
    fn new() -> Self {
        Self(reqwest::Client::new())
    }

    async fn resolve(&self, asset: &Asset) -> Result<String> {
        let response = self
            .0
            .get(format!("{}.sha256", asset.url))
            .send()
            .await?
            .text()
            .await?;
        self.parse_response(asset.name.as_str(), response.as_str())
    }

    fn parse_response(&self, asset_name: &str, response: &str) -> Result<String> {
        let parts: Vec<_> = response.split_whitespace().collect();
        if parts.len() != 2 {
            return Err(FetcherError::InvalidSha256(parts.len()));
        }

        let (sha256, filename) = (parts[0], parts[1]);
        match !filename.starts_with('*') || &filename[1..] != asset_name {
            false => Ok(sha256.to_string()),
            true => Err(FetcherError::WrongChecksum),
        }
    }
}

impl From<octocrab::Error> for FetcherError {
    fn from(err: octocrab::Error) -> Self {
        FetcherError::OctoError(err)
    }
}

impl From<reqwest::Error> for FetcherError {
    fn from(err: reqwest::Error) -> Self {
        FetcherError::ReqwestError(err)
    }
}

fn remove_game_suffix(asset_name: &str) -> &str {
    let platform = asset_name
        .find('.')
        .map_or(asset_name, |pos| &asset_name[..pos]);
    platform
        .find("_releasedbg")
        .map_or(platform, |pos| &platform[..pos])
}
