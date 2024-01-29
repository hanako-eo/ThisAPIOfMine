use std::collections::HashMap;
use std::sync::Mutex;

use actix_web::{get, middleware, web, App, HttpServer};
use actix_web::{HttpResponse, Responder};
use cached::{CachedAsync, TimedCache};
use game_data::{Asset, GameRelease};
use serde::Deserialize;

use crate::config::ApiConfig;
use crate::fetcher::Fetcher;
use crate::game_data::GameVersion;

mod config;
mod fetcher;
mod game_data;

#[derive(Deserialize)]
struct VersionQuery {
    platform: String,
}

struct AppData {
    cache: Mutex<TimedCache<&'static str, CachedReleased>>,
    config: ApiConfig,
    fetcher: Fetcher,
}

#[derive(Clone)]
enum CachedReleased {
    Updater(HashMap<String, Asset>),
    Game(Vec<GameRelease>),
}

#[get("/game_version")]
async fn game_version(
    app_data: web::Data<AppData>,
    ver_query: web::Query<VersionQuery>,
) -> impl Responder {
    let AppData {
        cache,
        config,
        fetcher,
    } = app_data.as_ref();
    let mut cache = cache.lock().unwrap();

    // TODO: remove .cloned
    let Ok(CachedReleased::Updater(updater_releases)) = cache.try_get_or_set_with("updater_releases", || async {
        fetcher.get_updater_releases().await.map(CachedReleased::Updater)
    }).await.cloned() else {
        return HttpResponse::InternalServerError().finish();
    };

    // TODO: remove .cloned
    let Ok(CachedReleased::Game(game_releases)) = cache.try_get_or_set_with("game_releases", || async {
        fetcher.get_game_releases().await.map(CachedReleased::Game)
    }).await.cloned() else {
        return HttpResponse::InternalServerError().finish();
    };

    let updater_filename = format!("{}_{}", &ver_query.platform, config.updater_filename);
    let game_release = game_releases.into_iter().rev().find_map(|release| {
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
    let config: ApiConfig = confy::load_path("tsom_api_config.toml").unwrap();
    let fetcher = Fetcher::from_config(&config).unwrap();

    std::env::set_var("RUST_LOG", "info,actix_web=info");
    env_logger::init();

    let bind_address = format!("{}:{}", config.listen_address, config.listen_port);

    let data_config = web::Data::new(AppData {
        cache: Mutex::new(TimedCache::with_lifespan(config.cache_lifespan)), // 5min
        config,
        fetcher,
    });

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
