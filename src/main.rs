use actix_web::{get, middleware, web, App, HttpServer};
use actix_web::{HttpResponse, Responder};
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
    config: ApiConfig,
    fetcher: Fetcher,
}

#[get("/game_version")]
async fn game_version(
    app_data: web::Data<AppData>,
    ver_query: web::Query<VersionQuery>,
) -> impl Responder {
    let AppData { config, fetcher } = app_data.as_ref();

    let Ok(updater_releases) = fetcher.get_updater_releases().await else {
        return HttpResponse::InternalServerError().finish();
    };

    let Ok(game_releases) = fetcher.get_game_releases().await else {
        return HttpResponse::InternalServerError().finish();
    };

    let game_release = game_releases.into_iter().rev().find_map(|release| {
        let updater_filename = format!("{}_{}", &ver_query.platform, config.updater_filename);

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

    let data_config = web::Data::new(AppData { config, fetcher });

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
