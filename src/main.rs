use std::collections::HashMap;

use actix_web::{get, middleware, web, App, HttpServer};
use actix_web::{HttpResponse, Responder};
use octocrab::models::repos;
use semver::Version;
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Serialize, Deserialize)]
struct AppConfig {
    listen_address: String,
    listen_port: u16,
    repo_owner: String,
    game_repository: String,
    updater_repository: String,
    updater_filename: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            listen_address: "0.0.0.0".to_string(),
            listen_port: 14770,
            repo_owner: "DigitalpulseSoftware".to_string(),
            game_repository: "ThisSpaceOfMine".to_string(),
            updater_filename: "this_updater_of_mine".to_string(),
            updater_repository: "ThisUpdaterOfMine".to_string(),
        }
    }
}

#[derive(Clone, Serialize)]
struct DownloadableAsset {
    download_url: String,
    sha256: Option<String>,
    size: i64,
}

struct GameRelease {
    assets: DownloadableAsset,
    assets_version: Version,
    binaries: HashMap<String, DownloadableAsset>,
    version: Version,
}

#[derive(Deserialize)]
struct VersionQuery {
    platform: String,
}

fn parse_response(asset_name: &str, response: &str) -> Option<String> {
    let parts: Vec<_> = response.split_whitespace().collect();
    if parts.len() != 2 {
        eprintln!(
            "unexpected part count (expected sha256+filename, got {} parts)",
            parts.len()
        );
        return None;
    }

    let (sha256, filename) = (parts[0], parts[1]);
    match !filename.starts_with('*') || &filename[1..] != asset_name {
        false => Some(sha256.to_string()),
        true => {
            eprintln!("checksum file has wrong filename (expected *{asset_name} got {filename})");
            None
        }
    }
}

async fn download_sha256(client: &reqwest::Client, url: &Url) -> Result<String, reqwest::Error> {
    client.get(url.clone()).send().await?.text().await
}

async fn download_checksum(
    client: &reqwest::Client,
    entry_name: &str,
    asset: &repos::Asset,
    hashes_assets: &HashMap<&str, &repos::Asset>,
) -> Option<String> {
    let Some(entry) = hashes_assets.get(entry_name) else {
        eprintln!("{entry_name} hash not found");
        return None;
    };

    match download_sha256(client, &entry.browser_download_url).await {
        Ok(response) => parse_response(asset.name.as_str(), response.as_str()),
        Err(err) => {
            eprintln!("failed to download sha256 of {entry_name}: {err}");
            None
        }
    }
}

async fn download_checksums<'a, 'b, B>(
    assets_asset: Option<&repos::Asset>,
    binary_assets: B,
    hashes_assets: HashMap<&'b str, &repos::Asset>,
) -> HashMap<&'b str, String>
where
    'b: 'a,
    B: Iterator<Item = (&'a &'b str, &'a &'b repos::Asset)>,
{
    let client = reqwest::Client::new();

    let mut hashes = HashMap::new();
    for (entry_name, asset) in binary_assets {
        if let Some(checksum) = download_checksum(&client, entry_name, asset, &hashes_assets).await
        {
            hashes.insert(*entry_name, checksum);
        }
    }

    if let Some(asset) = assets_asset {
        if let Some(checksum) = download_checksum(&client, "assets", asset, &hashes_assets).await {
            hashes.insert("assets", checksum);
        }
    }

    hashes
}

fn remove_game_suffix(asset_name: &str) -> &str {
    let platform = asset_name
        .find('.')
        .map_or(asset_name, |pos| &asset_name[..pos]);
    platform
        .find("_releasedbg")
        .map_or(platform, |pos| &platform[..pos])
}

async fn get_updater_releases(
    app_config: &AppConfig,
) -> Option<HashMap<String, DownloadableAsset>> {
    let output = octocrab::instance()
        .repos(&app_config.repo_owner, &app_config.updater_repository)
        .releases()
        .list()
        .send()
        .await;

    let releases = match output {
        Ok(output) => output,
        Err(err) => {
            eprintln!("failed to retrieve auto-updater releases: {}", err);
            return None;
        }
    };

    let Some(last_release) = releases.into_iter().find(|release| !release.prerelease) else {
        eprintln!("no auto-updater releases found");
        return None;
    };

    let (binary_assets, hashes_assets) = last_release.assets.iter().fold(
        (HashMap::new(), HashMap::new()),
        |(mut binary_assets, mut hashes_assets), asset| {
            let platform = remove_game_suffix(asset.name.as_str());

            let container_assets = match asset.name.ends_with(".sha256") {
                true => &mut hashes_assets,
                false => &mut binary_assets,
            };

            container_assets.insert(platform, asset);

            (binary_assets, hashes_assets)
        },
    );

    let hashes = download_checksums(None, binary_assets.iter(), hashes_assets).await;

    let binaries = binary_assets
        .into_iter()
        .map(|(platform, asset)| {
            (
                platform.to_string(),
                DownloadableAsset {
                    download_url: asset.browser_download_url.to_string(),
                    size: asset.size,
                    sha256: hashes.get(platform).cloned(),
                },
            )
        })
        .collect::<HashMap<String, DownloadableAsset>>();

    Some(binaries)
}

#[derive(Serialize)]
struct GameVersion {
    assets: DownloadableAsset,
    assets_version: String,
    binaries: DownloadableAsset,
    updater: DownloadableAsset,
    version: String,
}

async fn get_game_releases(app_config: &AppConfig) -> Option<Vec<GameRelease>> {
    let output = octocrab::instance()
        .repos(&app_config.repo_owner, &app_config.game_repository)
        .releases()
        .list()
        .send()
        .await;

    let releases = match output {
        Ok(output) => output,
        Err(err) => {
            eprintln!("failed to retrieve game releases: {}", err);
            return None;
        }
    };

    let mut assets_info = None;
    let mut assets_version = None;

    let mut game_releases = Vec::new();
    for release in releases.into_iter().rev() {
        if release.prerelease {
            continue;
        }

        let version = match Version::parse(&release.tag_name) {
            Ok(version) => version,
            Err(err) => {
                eprintln!(
                    "failed to parse version of tag {0}: {1}",
                    release.tag_name, err
                );
                continue;
            }
        };

        let (binary_assets, hashes_assets, assets_asset) = release.assets.iter().fold(
            (HashMap::new(), HashMap::new(), None),
            |(mut binary_assets, mut hashes_assets, assets_asset), asset| {
                let platform = remove_game_suffix(asset.name.as_str());

                if asset.name.ends_with(".sha256") {
                    hashes_assets.insert(platform, asset);
                } else if platform == "assets" {
                    return (binary_assets, hashes_assets, Some(asset));
                } else {
                    binary_assets.insert(platform, asset);
                }

                (binary_assets, hashes_assets, assets_asset)
            },
        );

        let hashes = download_checksums(assets_asset, binary_assets.iter(), hashes_assets).await;

        // Build assets
        if let Some(asset) = assets_asset {
            assets_info = Some(DownloadableAsset {
                download_url: asset.browser_download_url.to_string(),
                size: asset.size,
                sha256: hashes.get("assets").cloned(),
            });
            assets_version = Some(version.clone());
        }

        // Build binaries
        let binaries = binary_assets
            .into_iter()
            .map(|(platform, asset)| {
                (
                    platform.to_string(),
                    DownloadableAsset {
                        download_url: asset.browser_download_url.to_string(),
                        size: asset.size,
                        sha256: hashes.get(platform).cloned(),
                    },
                )
            })
            .collect::<HashMap<String, DownloadableAsset>>();

        let (Some(assets_info), Some(assets_version)) = (&assets_info, &assets_version) else {
            // if we don't have assets URL/versions yet we must skip
            eprintln!("ignoring release {version} because no assets was found");
            continue;
        };

        game_releases.push(GameRelease {
            assets: assets_info.clone(),
            assets_version: assets_version.clone(),
            binaries,
            version,
        });
    }

    Some(game_releases)
}

#[get("/game_version")]
async fn game_version(
    app_config: web::Data<AppConfig>,
    ver_query: web::Query<VersionQuery>,
) -> impl Responder {
    let Some(updater_releases) = get_updater_releases(&app_config).await else {
        return HttpResponse::InternalServerError().finish();
    };

    let Some(game_releases) = get_game_releases(&app_config).await else {
        return HttpResponse::InternalServerError().finish();
    };

    let game_release = game_releases.into_iter().rev().find_map(|release| {
        let updater_filename = format!("{}_{}", &ver_query.platform, app_config.updater_filename);

        let updater_release = updater_releases.get(&updater_filename)?;

        release
            .binaries
            .get(&ver_query.platform)
            .map(|binaries| GameVersion {
                assets: release.assets,
                assets_version: release.assets_version.to_string(),
                binaries: binaries.clone(),
                updater: updater_release.clone(),
                version: release.version.to_string(),
            })
    });

    match game_release {
        Some(response) => HttpResponse::Ok().json(web::Json(response)),
        None => {
            eprintln!(
                "no updater release found for platform {}",
                ver_query.platform
            );
            HttpResponse::NotFound().finish()
        }
    }
}

#[actix_web::main]
async fn main() -> Result<(), std::io::Error> {
    let config: AppConfig = confy::load_path("tsom_api_config.toml").unwrap();

    std::env::set_var("RUST_LOG", "info,actix_web=info");
    env_logger::init();

    let bind_address = format!("{}:{}", config.listen_address, config.listen_port);

    let data_config = web::Data::new(config);
    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .app_data(data_config.clone())
            .service(game_version)
    })
    .bind(bind_address)?
    .run()
    .await
}
