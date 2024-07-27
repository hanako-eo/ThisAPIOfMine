pub use checksum::{ChecksumFetcher, HttpChecksumFetcher};
use futures::future::join_all;
use octocrab::models::repos;
use octocrab::{Octocrab, OctocrabBuilder};
pub use repo::RepoFetcher;
use semver::Version;

use crate::config::ApiConfig;
use crate::errors::{InternalError, Result};
use crate::game_data::{Asset, Assets, GameRelease, Repo};

mod checksum;
mod repo;
#[cfg(test)]
mod tests;

pub struct Fetcher<F: RepoFetcher, C: ChecksumFetcher> {
    game_repo: Repo,
    updater_repo: Repo,

    repo_fetcher: F,
    checksum_fetcher: C,
}

impl Fetcher<Octocrab, HttpChecksumFetcher> {
    pub fn from_config(config: &ApiConfig) -> Result<Self> {
        let mut octocrab = OctocrabBuilder::default();
        if let Some(github_pat) = &config.github_pat {
            octocrab = octocrab.personal_token(github_pat.unsecure().to_string());
        }

        Ok(Self::new(
            Repo::new(&config.repo_owner, &config.game_repository),
            Repo::new(&config.repo_owner, &config.updater_repository),
            octocrab.build()?,
            HttpChecksumFetcher::new(),
        ))
    }
}

impl<F: RepoFetcher, C: ChecksumFetcher> Fetcher<F, C> {
    pub fn new(game_repo: Repo, updater_repo: Repo, repo_fetcher: F, checksum_fetcher: C) -> Self {
        Self {
            game_repo,
            updater_repo,
            repo_fetcher,
            checksum_fetcher,
        }
    }

    pub async fn get_latest_game_release(&self) -> Result<GameRelease> {
        let releases = self.repo_fetcher.get_releases(&self.game_repo).await?;

        let mut versions_released = releases
            .into_iter()
            .filter(|r| !r.prerelease)
            .filter_map(|r| Version::parse(&r.tag_name).ok().map(|v| (v, r)));

        let Some((latest_version, latest_release)) = versions_released.next() else {
            return Err(InternalError::NoReleaseFound);
        };

        let mut binaries = self
            .get_assets_and_checksums(&latest_release.assets, &latest_version, None)
            .await
            .map(|((platform, mut asset), sha256)| {
                asset.sha256 = match sha256 {
                    Ok(sha256) => Some(sha256),
                    Err(err) => match err.is::<reqwest::Error>() {
                        true => None,
                        false => return Err(err),
                    },
                };

                Ok((platform.to_string(), asset))
            })
            .collect::<Result<Assets>>()?;

        for (version, release) in versions_released {
            for ((platform, mut asset), sha256) in self
                .get_assets_and_checksums(&release.assets, &version, Some(&binaries))
                .await
            {
                asset.sha256 = match sha256 {
                    Ok(sha256) => Some(sha256),
                    Err(err) => match err.is::<reqwest::Error>() {
                        true => None,
                        false => return Err(err),
                    },
                };

                binaries.insert(platform.to_string(), asset);
            }
        }

        let latest_assets = binaries.remove("assets");

        match latest_assets {
            Some(assets) => Ok(GameRelease {
                assets_version: assets.version.clone(),
                assets,
                binaries,
                version: latest_version,
            }),
            None => Err(InternalError::NoReleaseFound),
        }
    }

    pub async fn get_latest_updater_release(&self) -> Result<Assets> {
        let last_release = self
            .repo_fetcher
            .get_last_release(&self.updater_repo)
            .await?;

        let version = Version::parse(&last_release.tag_name)?;

        self.get_assets_and_checksums(&last_release.assets, &version, None)
            .await
            .map(|((platform, mut asset), sha256)| {
                asset.sha256 = match sha256 {
                    Ok(sha256) => Some(sha256),
                    Err(err) => match err.is::<reqwest::Error>() {
                        true => None,
                        false => return Err(err),
                    },
                };

                Ok((platform.to_string(), asset))
            })
            .collect::<Result<Assets>>()
    }

    async fn get_assets_and_checksums<'a: 'b, 'b, A>(
        &self,
        assets: A,
        version: &Version,
        binaries: Option<&Assets>,
    ) -> impl Iterator<Item = ((&'b str, Asset), Result<String>)>
    where
        A: IntoIterator<Item = &'a repos::Asset>,
    {
        let assets = assets
            .into_iter()
            .filter_map(|asset| {
                let platform = remove_game_suffix(asset.name.as_str());
                match !asset.name.ends_with(".sha256")
                    && !binaries.is_some_and(|b| b.contains_key(platform))
                {
                    true => Some((platform, Asset::with_version(asset, version.clone()))),
                    false => None,
                }
            })
            .collect::<Vec<(&str, Asset)>>();

        let checksums = join_all(
            assets
                .iter()
                .map(|(_, asset)| self.checksum_fetcher.resolve_asset(asset)),
        )
        .await;

        assets.into_iter().zip(checksums)
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
