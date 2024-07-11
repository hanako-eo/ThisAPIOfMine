use actix_governor::{Governor, GovernorConfig, GovernorConfigBuilder};
use actix_web::{middleware, web, App, HttpServer};
use cached::TimedCache;
use confy::ConfyError;
use std::sync::Mutex;

use crate::app_data::AppData;
use crate::config::ApiConfig;
use crate::fetcher::Fetcher;
use crate::players::{player_authenticate, player_create};
use crate::version::game_version;

mod app_data;
mod config;
mod errors;
mod fetcher;
mod game_data;
mod players;
mod version;

use tokio_postgres::NoTls;

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
            err.kind()
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

    let governor_conf = GovernorConfig::default();

    let player_create_governor_conf = GovernorConfigBuilder::default()
        .per_second(10)
        .burst_size(1)
        .finish()
        .unwrap();

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .wrap(Governor::new(&governor_conf))
            .app_data(data_config.clone())
            .app_data(pg_pool.clone())
            .service(game_version)
            .service(player_authenticate)
            .service(
                web::scope("")
                    .wrap(Governor::new(&player_create_governor_conf))
                    .service(player_create),
            )
    })
    .bind(bind_address)?
    .run()
    .await
}
