use std::collections::HashMap;

use mockall::predicate::eq;
use octocrab::models::repos::{Asset as RepoAsset, Release};
use octocrab::models::{AssetId, ReleaseId};
use semver::Version;
use url::Url;

use crate::errors::Result;
use crate::game_data::{Asset, Repo};

use super::checksum::MockChecksumFetcher;
use super::repo::MockRepoFetcher;
use super::Fetcher;

#[tokio::test]
async fn retrieve_the_latest_version_of_the_updater_when_there_is_only_one_available() -> Result<()> {
    let updater_repo = Repo::new("repo", "updater");
    let game_repo = Repo::new("repo", "game");
    let windows_asset = |sha256| Asset {
        size: 1_000_000,
        name: "windows_x64_releasedbg.zip".to_string(),
        version: Version::new(0, 1, 0),
        download_url: "http://github.com/repo/updater/releases/0.1.0/windows_x64_releasedbg.zip"
            .to_string(),
        sha256,
    };

    let mut repo_fetcher = MockRepoFetcher::new();
    let mut checksum_fetcher = MockChecksumFetcher::new();

    repo_fetcher
        .expect_get_last_release()
        .with(eq(updater_repo.clone()))
        .times(1)
        .returning(|repo| {
            release_builder(|release| {
                release.tag_name = "0.1.0".to_string();
                release.assets = vec![
                    asset_builder(|asset| {
                        asset.size = 1_000_000;
                        asset.name = "windows_x64_releasedbg.zip".to_string();
                        asset.browser_download_url = Url::parse(&format!(
                            "http://github.com/{}/{}/releases/0.1.0/{}",
                            repo.owner(),
                            repo.repository(),
                            asset.name
                        ))?;

                        Ok(())
                    })?,
                    asset_builder(|asset| {
                        asset.size = 93;
                        asset.name = "windows_x64_releasedbg.zip.sha256".to_string();
                        asset.browser_download_url = Url::parse(&format!(
                            "http://github.com/{}/{}/releases/0.1.0/{}",
                            repo.owner(),
                            repo.repository(),
                            asset.name
                        ))?;

                        Ok(())
                    })?,
                ];

                Ok(())
            })
        });

    checksum_fetcher
        .expect_resolve_asset()
        .with(eq(windows_asset(None)))
        .returning(|_| Ok("*sha256-key*".to_string()));

    let fetcher = Fetcher::new(
        game_repo.clone(),
        updater_repo.clone(),
        repo_fetcher,
        checksum_fetcher,
    );

    let latest_releases = fetcher.get_latest_updater_release().await.expect("fail :(");

    assert_eq!(
        latest_releases,
        HashMap::from_iter([(
            "windows_x64".to_string(),
            windows_asset(Some("*sha256-key*".to_string()))
        )])
    );

    Ok(())
}

fn asset_builder<B>(builder: B) -> Result<RepoAsset>
where
    B: FnOnce(&mut RepoAsset) -> Result<()>,
{
    let mut asset = RepoAsset {
        url: Url::parse("http://exemple.com")?,
        browser_download_url: Url::parse("http://exemple.com")?,
        id: AssetId(0),
        node_id: String::new(),
        name: String::new(),
        label: None,
        state: String::new(),
        content_type: String::new(),
        size: 0,
        download_count: 0,
        created_at: Default::default(),
        updated_at: Default::default(),
        uploader: None,
    };

    builder(&mut asset)?;
    Ok(asset)
}

fn release_builder<B>(builder: B) -> Result<Release>
where
    B: FnOnce(&mut Release) -> Result<()>,
{
    let mut release = Release {
        url: Url::parse("http://exemple.com")?,
        html_url: Url::parse("http://exemple.com")?,
        assets_url: Url::parse("http://exemple.com")?,
        upload_url: String::new(),
        tarball_url: None,
        zipball_url: None,
        id: ReleaseId(0),
        node_id: String::new(),
        tag_name: String::new(),
        target_commitish: String::new(),
        name: None,
        body: None,
        draft: false,
        prerelease: false,
        created_at: None,
        published_at: None,
        author: None,
        assets: Vec::new(),
    };

    builder(&mut release);
    Ok(release)
}
