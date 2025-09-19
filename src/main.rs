#[macro_use]
extern crate rocket;

use nanoid::nanoid;
use redb::{Database, ReadableTable, TableDefinition};
use rocket::State;
use rocket::http::Status;
use rocket::response::Redirect;
use rocket::serde::json::{Value, json};
use service::PastyError;
use std::convert::TryInto;
use url::Url;

mod store;
use store::*;

mod config;
use config::*;

mod service;

#[derive(Debug, Responder)]
enum Response {
    Json(Value),
    Plaintext(String),
    Redirect(Box<Redirect>),
}

#[get("/")]
fn get_index(config: &State<Config>) -> Response {
    if !config.index_link.is_empty() {
        Response::Redirect(Box::new(Redirect::to(config.index_link.clone())))
    } else {
        Response::Json(json!({
            "status": "200",
            "message": config.index_text
        }))
    }
}

macro_rules! json_with_status {
    ($status:expr, $message:expr) => {
        (
            $status,
            Response::Json(json!({
                "status": $status.code,
                "message": $message
            }))
        )
    };
}

fn handle_pasty_error(err: anyhow::Error) -> (Status, Response) {
    match err.downcast_ref::<PastyError>() {
        Some(PastyError::NotFound) => {
            json_with_status!(Status::NotFound, "The short link does not exist.")
        }
        Some(PastyError::TokenRequired) => json_with_status!(
            Status::BadRequest,
            "An access token is required for this action."
        ),
        Some(PastyError::TokenMismatch) => {
            json_with_status!(Status::BadRequest, "The access token is incorrect.")
        }
        _ => json_with_status!(
            Status::InternalServerError,
            format!("Internal server error: {}", err)
        ),
    }
}

fn check_access(config: &State<Config>, access: &str) -> Option<(Status, Response)> {
    if !config.access_password.is_empty() && access != config.access_password {
        return Some(json_with_status!(
            Status::Unauthorized,
            "Access password is incorrect."
        ));
    }

    None
}

const ALPHABET: [char; 62] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i',
    'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', 'A', 'B',
    'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U',
    'V', 'W', 'X', 'Y', 'Z',
];

#[post("/?<type>&<pwd>&<access>", data = "<content>")]
fn post_index(
    db: &State<Database>,
    config: &State<Config>,
    r#type: Option<&str>,
    pwd: Option<&str>,
    access: &str,
    content: &str,
) -> (Status, Response) {
    let length: usize = config.random_id_length.try_into().unwrap();
    let id = nanoid!(length, &ALPHABET);
    post_by_id(db, config, &id, r#type, pwd, access, content)
}

#[get("/<id>")]
fn get_by_id(db: &State<Database>, id: &str) -> (Status, Response) {
    match service::get_pasty_by_id(db, id) {
        Ok(pasty) => {
            service::view_stats_by_id(db, id).ok();

            match pasty.content_type {
                ContentType::Plaintext => (Status::Ok, Response::Plaintext(pasty.content)),
                ContentType::Redirect => (
                    Status::Found,
                    Response::Redirect(Box::new(Redirect::to(pasty.content))),
                ),
            }
        }
        Err(err) => handle_pasty_error(err),
    }
}

#[get("/<id>/stats")]
fn get_stat_by_id(db: &State<Database>, id: &str) -> (Status, Response) {
    match service::get_stats_by_id(db, id) {
        Ok(stats) => {
            let views = stats.views;
            let created_at = stats.created_at.to_rfc3339();
            let updated_at = stats.updated_at.to_rfc3339();
            let last_viewed_at = stats.last_viewed_at.to_rfc3339();
            (
                Status::Ok,
                Response::Json(json!({
                    "id": id,
                    "views": views,
                    "created_at": created_at,
                    "updated_at": updated_at,
                    "last_viewed_at": last_viewed_at
                })),
            )
        }
        Err(err) => handle_pasty_error(err),
    }
}

#[get("/all?<access>")]
fn get_all(db: &State<Database>, config: &State<Config>, access: &str) -> (Status, Response) {
    if let Some(response) = check_access(config, access) {
        return response;
    }

    let pasties = match service::list_all_pasties(db) {
        Ok(pasties) => pasties,
        Err(err) => return handle_pasty_error(err),
    };

    (
        Status::Ok,
        Response::Json(json!(
            pasties
                .into_iter()
                .map(|(pasty, stats)| {
                    json!({
                        "id": pasty.id,
                        "content_type": pasty.content_type,
                        "views": stats.views,
                        "created_at": stats.created_at.to_rfc3339(),
                        "updated_at": stats.updated_at.to_rfc3339(),
                        "last_viewed_at": stats.last_viewed_at.to_rfc3339(),
                        "content": pasty.content
                    })
                })
                .collect::<Vec<Value>>()
        )),
    )
}

#[post("/<id>?<type>&<pwd>&<access>", data = "<content>")]
fn post_by_id(
    db: &State<Database>,
    config: &State<Config>,
    id: &str,
    r#type: Option<&str>,
    pwd: Option<&str>,
    access: &str,
    content: &str,
) -> (Status, Response) {
    if let Some(response) = check_access(config, access) {
        return response;
    }

    if id.is_empty() {
        return json_with_status!(Status::BadRequest, "Missing parameter");
    }

    let content_type = match r#type {
        Some("link") => ContentType::Redirect,
        Some("plain") => ContentType::Plaintext,
        None => ContentType::Plaintext,
        _ => return json_with_status!(Status::BadRequest, "Unsupported short link type"),
    };

    if content_type == ContentType::Redirect && Url::parse(content).is_err() {
        return json_with_status!(Status::BadRequest, "The given link is not a valid URL");
    }

    match service::update_pasty_by_id(db, id, content, content_type, pwd) {
        Ok(_) => json_with_status!(
            Status::Ok,
            format!("The short link has been updated: {}", id)
        ),
        Err(err) => handle_pasty_error(err),
    }
}

#[delete("/<id>?<pwd>&<access>")]
fn delete_by_id(
    db: &State<Database>,
    config: &State<Config>,
    id: &str,
    pwd: Option<&str>,
    access: &str,
) -> (Status, Response) {
    if let Some(response) = check_access(config, access) {
        return response;
    }

    match service::delete_pasty_by_id(db, id, pwd) {
        Ok(_) => json_with_status!(
            Status::Ok,
            format!("The short link has been deleted: {}", id)
        ),
        Err(err) => handle_pasty_error(err),
    }
}

#[catch(404)]
fn not_found() -> (Status, Response) {
    json_with_status!(Status::NotFound, "The requested resource was not found.")
}

#[catch(500)]
fn internal_error() -> (Status, Response) {
    json_with_status!(Status::InternalServerError, "Internal server error.")
}

#[rocket::main]
async fn main() {
    let rocket_instance = rocket::build();
    let figment = rocket_instance.figment();
    let config: Config = figment
        .extract_inner("pasty")
        .expect("error loading configuration");

    let mut db = Database::create(config.db_path.clone()).expect("error opening database");
    db.upgrade().expect("error upgrading database");

    service::ensure_table_exists(&db).expect("error ensuring table exists");

    let result = rocket_instance
        .manage(db)
        .manage(config)
        .register("/", catchers![not_found, internal_error])
        .mount(
            "/",
            routes![
                get_all,
                get_stat_by_id,
                get_index,
                get_by_id,
                post_index,
                post_by_id,
                delete_by_id
            ],
        )
        .launch()
        .await;

    result.expect("error shutting down http server");
}
