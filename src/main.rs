use std::collections::HashMap;
use std::sync::Mutex;

use actix_web::{get, middleware, web, App, HttpServer};
use actix_web::{HttpResponse, Responder};
use cached::{CachedAsync, TimedCache};
use confy::ConfyError;
use game_data::{Asset, GameRelease};
use serde::Deserialize;

use crate::app_data::AppData;
use crate::config::ApiConfig;
use crate::fetcher::Fetcher;
use crate::game_data::GameVersion;
use crate::players::create_player;

mod app_data;
mod config;
mod errors;
mod fetcher;
mod game_data;
mod players;

use tokio_postgres::NoTls;

#[derive(Deserialize)]
struct VersionQuery {
    platform: String,
}

#[derive(Clone)]
enum CachedReleased {
    Updater(HashMap<String, Asset>),
    Game(GameRelease),
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
    let Ok(CachedReleased::Updater(updater_release)) = cache
        .try_get_or_set_with("latest_updater_release", || async {
            fetcher
                .get_latest_updater_release()
                .await
                .map(CachedReleased::Updater)
        })
        .await
        .cloned()
    else {
        return HttpResponse::InternalServerError().finish();
    };

    // TODO: remove .cloned
    let Ok(CachedReleased::Game(game_release)) = cache
        .try_get_or_set_with("latest_game_release", || async {
            fetcher
                .get_latest_game_release()
                .await
                .map(CachedReleased::Game)
        })
        .await
        .cloned()
    else {
        return HttpResponse::InternalServerError().finish();
    };

    let updater_filename = format!("{}_{}", ver_query.platform, config.updater_filename);

    let (Some(updater), Some(binary)) = (
        updater_release.get(&updater_filename),
        game_release.binaries.get(&ver_query.platform),
    ) else {
        eprintln!(
            "no updater or game binary release found for platform {}",
            ver_query.platform
        );
        return HttpResponse::NotFound().finish();
    };

    HttpResponse::Ok().json(web::Json(GameVersion {
        assets: game_release.assets,
        assets_version: game_release.assets_version.to_string(),
        binaries: binary.clone(),
        updater: updater.clone(),
        version: game_release.version.to_string(),
    }))
}

fn setup_pg_pool(api_config: &ApiConfig) -> deadpool_postgres::Pool {
    use deadpool_postgres::{Config, ManagerConfig, RecyclingMethod, Runtime};

    let mut pg_config = Config::new();
    pg_config.host = Some(api_config.db_host.clone());
    pg_config.password = Some(api_config.db_password.unsecure().to_string());
    pg_config.user = Some(api_config.db_user.clone());
    pg_config.dbname = Some(api_config.db_database.clone());
    pg_config.manager = Some(ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    });

    pg_config.create_pool(Some(Runtime::Tokio1), NoTls).unwrap()
}

#[actix_web::main]
async fn main() -> Result<(), std::io::Error> {
    let config = match confy::load_path("tsom_api_config.toml") {
        Ok(config) => config,
        Err(ConfyError::BadTomlData(err)) => panic!(
            "an error occured on the parsing of the file tsom_api_config.toml:\n{}",
            err.message()
        ),
        Err(ConfyError::GeneralLoadError(err)) => panic!(
            "an error occured on the loading of the file tsom_api_config.toml:\n{}",
            err
        ),
        Err(_) => panic!(
            "wrong data in the file, failed to load config, please check tsom_api_config.toml"
        ),
    };
    let fetcher = Fetcher::from_config(&config).unwrap();

    let pg_pool = web::Data::new(setup_pg_pool(&config));

    // Try to connect to database
    let test_client = pg_pool.get().await.expect("failed to connect to database");
    drop(test_client);

    std::env::set_var("RUST_LOG", "debug,actix_web=debug");
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
            .app_data(pg_pool.clone())
            .service(game_version)
            .service(create_player)
    })
    .bind(bind_address)?
    .run()
    .await
}
