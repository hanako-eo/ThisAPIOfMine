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
            listen_address: "0.0.0.0".to_owned(),
            listen_port: 14770,
            repo_owner: "DigitalpulseSoftware".to_owned(),
            game_repository: "ThisSpaceOfMine".to_owned(),
            updater_filename: "this_updater_of_mine".to_owned(),
            updater_repository: "ThisUpdaterOfMine".to_owned(),
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
        println!(
            "unexpected part count (expected sha256+filename, got {} parts)",
            parts.len()
        );
        return None;
    }

    let sha256 = parts[0];
    let filename = parts[1];
    if !filename.starts_with('*') || &filename[1..] != asset_name {
        println!("checksum file has wrong filename (expected *{asset_name} got {filename})");
        return None;
    }

    Some(sha256.to_string())
}

async fn download_sha256(client: &reqwest::Client, url: &Url) -> Result<String, reqwest::Error> {
    client.get(url.clone()).send().await?.text().await
}

async fn download_checksum<'a>(
    client: &reqwest::Client,
    entry_name: &'a str,
    asset: &repos::Asset,
    hashes: &mut HashMap<&'a str, String>,
    hashes_assets: &HashMap<&str, &repos::Asset>,
) {
    let checksum: Option<String> = match hashes_assets.get(entry_name) {
        Some(entry) => match download_sha256(client, &entry.browser_download_url).await {
            Ok(response) => parse_response(&asset.name, &response),
            Err(err) => {
                println!("failed to download sha256 of {entry_name}: {err}");
                None
            }
        },
        None => {
            println!("{entry_name} hash not found");
            None
        }
    };

    if let Some(hash) = checksum {
        hashes.insert(entry_name, hash);
    }
}

async fn download_checksums<'a>(
    assets_asset: Option<&repos::Asset>,
    binary_assets: &HashMap<&'a str, &repos::Asset>,
    hashes_assets: &HashMap<&str, &repos::Asset>,
) -> HashMap<&'a str, String> {
    let client = reqwest::Client::new();

    let mut hashes = HashMap::new();
    for (entry_name, asset) in binary_assets {
        download_checksum(&client, entry_name, asset, &mut hashes, hashes_assets).await;
    }

    if let Some(asset) = assets_asset {
        download_checksum(&client, "assets", asset, &mut hashes, hashes_assets).await;
    }

    hashes
}

fn remove_game_suffix(asset_name: &str) -> &str {
    let mut platform = asset_name;
    platform = platform.find('.').map_or(platform, |pos| &platform[..pos]);
    platform = platform
        .find("_releasedbg")
        .map_or(platform, |pos| &platform[..pos]);

    platform
}

async fn get_updater_releases(
    app_config: &AppConfig,
) -> Option<HashMap<String, DownloadableAsset>> {
    let octo = octocrab::instance();
    let output = octo
        .repos(&app_config.repo_owner, &app_config.updater_repository)
        .releases()
        .list()
        .send()
        .await;

    if output.is_err() {
        println!(
            "failed to retrieve auto-updater releases: {}",
            output.unwrap_err()
        );
        return None;
    }
    let releases = output.unwrap();

    let last_release = releases.into_iter().find(|release| !release.prerelease);

    if last_release.is_none() {
        println!("no auto-updater releases found");
        return None;
    }
    let last_release = last_release.unwrap();

    let mut binary_assets = HashMap::new();
    let mut hashes_assets = HashMap::new();

    for asset in &last_release.assets {
        let platform = remove_game_suffix(&asset.name);
        if asset.name.ends_with(".sha256") {
            hashes_assets.insert(platform, asset);
        } else {
            binary_assets.insert(platform, asset);
        }
    }

    let hashes = download_checksums(None, &binary_assets, &hashes_assets).await;

    let mut binaries = HashMap::new();
    for (platform, asset) in binary_assets {
        binaries.insert(
            platform.to_string(),
            DownloadableAsset {
                download_url: asset.browser_download_url.to_string(),
                size: asset.size,
                sha256: hashes.get(platform).map(String::clone),
            },
        );
    }

    Some(binaries)
}

#[derive(Serialize)]
struct GameVersion<'a> {
    assets: &'a DownloadableAsset,
    assets_version: String,
    binaries: &'a DownloadableAsset,
    updater: &'a DownloadableAsset,
    version: String,
}

async fn get_game_releases(app_config: &AppConfig) -> Option<Vec<GameRelease>> {
    let octo = octocrab::instance();
    let output = octo
        .repos(&app_config.repo_owner, &app_config.game_repository)
        .releases()
        .list()
        .send()
        .await;

    if output.is_err() {
        println!("failed to retrieve game releases: {}", output.unwrap_err());
        return None;
    }
    let releases = output.unwrap();

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
                println!(
                    "failed to parse version of tag {0}: {1}",
                    release.tag_name, err
                );
                continue;
            }
        };

        let mut binary_assets = HashMap::new();
        let mut hashes_assets = HashMap::new();

        let mut assets_asset = None;

        for asset in &release.assets {
            let platform = remove_game_suffix(&asset.name);
            if asset.name.ends_with(".sha256") {
                hashes_assets.insert(platform, asset);
            } else if platform == "assets" {
                assets_asset = Some(asset);
            } else {
                binary_assets.insert(platform, asset);
            }
        }

        let hashes = download_checksums(assets_asset, &binary_assets, &hashes_assets).await;

        // Build assets
        if let Some(asset) = assets_asset {
            assets_info = Some(DownloadableAsset {
                download_url: asset.browser_download_url.to_string(),
                size: asset.size,
                sha256: hashes.get("assets").map(String::clone),
            });
            assets_version = Some(version.clone());
        }

        // Build binaries
        let mut binaries = HashMap::new();
        for (platform, asset) in binary_assets {
            binaries.insert(
                platform.to_string(),
                DownloadableAsset {
                    download_url: asset.browser_download_url.to_string(),
                    size: asset.size,
                    sha256: hashes.get(platform).map(String::clone),
                },
            );
        }

        // if we don't have assets URL/versions yet we must skip
        if assets_info.is_none() || assets_version.is_none() {
            println!("ignoring release {version} because no assets was found");
            continue;
        }

        game_releases.push(GameRelease {
            assets: assets_info.as_ref().unwrap().clone(),
            assets_version: assets_version.as_ref().unwrap().clone(),
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
    let updater_releases = match get_updater_releases(&app_config).await {
        Some(release) => release,
        None => {
            return HttpResponse::InternalServerError().finish();
        }
    };

    match get_game_releases(&app_config).await {
        Some(game_releases) => {
            for release in game_releases.iter().rev() {
                let updater_filename =
                    format!("{}_{}", &ver_query.platform, app_config.updater_filename);
                let updater_release = match updater_releases.get(&updater_filename) {
                    Some(release) => release,
                    None => {
                        println!(
                            "no updater release found for platform {}",
                            ver_query.platform
                        );
                        return HttpResponse::InternalServerError().finish();
                    }
                };

                if let Some(binaries) = release.binaries.get(&ver_query.platform) {
                    let reponse = GameVersion {
                        assets: &release.assets,
                        assets_version: release.assets_version.to_string(),
                        binaries,
                        updater: updater_release,
                        version: release.version.to_string(),
                    };
                    return HttpResponse::Ok().json(web::Json(reponse));
                }
            }

            HttpResponse::NotFound().finish()
        }
        None => HttpResponse::InternalServerError().finish(),
    }
}

#[actix_web::main]
async fn main() -> Result<(), std::io::Error> {
    let config: AppConfig = confy::load_path("tsom_api_config.toml").unwrap();

    std::env::set_var("RUST_LOG", "info,actix_web=info");
    env_logger::init();

    let bind_address = format!("{}:{}", &config.listen_address, &config.listen_port);

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .app_data(web::Data::new(config.clone()))
            .service(game_version)
    })
    .bind(bind_address)?
    .run()
    .await
}
