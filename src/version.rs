use std::collections::HashMap;

use actix_web::{get, web};
use actix_web::{HttpResponse, Responder};
use cached::CachedAsync;
use serde::Deserialize;

use crate::app_data::AppData;
use crate::game_data::{Asset, GameRelease, GameVersion};

#[derive(Deserialize)]
struct VersionQuery {
    platform: String,
}

#[derive(Clone)]
pub(crate) enum CachedReleased {
    Updater(HashMap<String, Asset>),
    Game(GameRelease),
}

#[get("/game_version")]
async fn route_game_version(
    app_data: web::Data<AppData>,
    ver_query: web::Query<VersionQuery>,
) -> impl Responder {
    let AppData {
        cache,
        config,
        fetcher,
    } = app_data.as_ref();
    let mut cache = cache.lock().await;

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
