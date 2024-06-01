#[macro_use]
extern crate rocket;

use nanoid::nanoid;
use redb::{Database, ReadableTable, TableDefinition};
use rocket::http::Status;
use rocket::response::Redirect;
use rocket::State;
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
    PlainText(String),
    Error(String),
    Redirect(Box<Redirect>),
}

#[get("/")]
fn get_index(config: &State<Config>) -> Response {
    if !config.index_link.is_empty() {
        Response::Redirect(Box::new(Redirect::to(config.index_link.clone())))
    } else {
        Response::PlainText(config.index_text.clone())
    }
}

fn handle_pasty_error(err: anyhow::Error) -> (Status, Response) {
    match err.downcast_ref::<PastyError>() {
        Some(PastyError::NotFound) => (
            Status::NotFound,
            Response::Error("此短链接不存在".to_string()),
        ),
        Some(PastyError::TokenRequired) => (
            Status::BadRequest,
            Response::Error("此短链接需要访问密码".to_string()),
        ),
        Some(PastyError::TokenMismatch) => (
            Status::BadRequest,
            Response::Error("访问密码错误".to_string()),
        ),
        _ => (
            Status::InternalServerError,
            Response::Error(format!("服务器内部错误：{}", err)),
        ),
    }
}

fn check_access(config: &State<Config>, access: &str) -> Option<(Status, Response)> {
    if !config.access_password.is_empty() && access != config.access_password {
        Some((
            Status::Unauthorized,
            Response::Error("访问密码错误".to_string()),
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
                ContentType::Plaintext => (Status::Ok, Response::PlainText(pasty.content)),
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
                Response::PlainText(format!(
                    "短链接 {} 的统计信息：\n\n\
                    - 访问次数：{}\n\
                    - 创建时间：{}\n\
                    - 更新时间：{}\n\
                    - 最后访问时间：{}",
                    id, views, created_at, updated_at, last_viewed_at
                )),
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

    let mut response = String::new();

    for (pasty, stats) in pasties {
        response.push_str(&format!(
            "短链接：{}\t类型：{:?}\t访问次数：{}\n\
            创建时间：{}\n\
            更新时间：{}\n\
            最后访问时间：{}\n\
            内容：{}\n\n",
            pasty.id,
            pasty.content_type,
            stats.views,
            stats.created_at.to_rfc3339(),
            stats.updated_at.to_rfc3339(),
            stats.last_viewed_at.to_rfc3339(),
            pasty.content
        ));
    }

    (Status::Ok, Response::PlainText(response))
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
        return (Status::BadRequest, Response::Error("缺少参数".to_string()));
    }

    let content_type = match r#type {
        Some("link") => ContentType::Redirect,
        Some("plain") => ContentType::Plaintext,
        None => ContentType::Plaintext,
        _ => {
            return (
                Status::BadRequest,
                Response::Error("不支持的短链接类型".to_string()),
            )
        }
    };

    if content_type == ContentType::Redirect && Url::parse(content).is_err() {
        return (
            Status::BadRequest,
            Response::Error("给定的链接不是有效的 URL".to_string()),
        );
    }

    match service::update_pasty_by_id(db, id, content, content_type, pwd) {
        Ok(_) => (
            Status::Ok,
            Response::PlainText(format!("更新数据成功：{}", id)),
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
        Ok(_) => (
            Status::Ok,
            Response::PlainText(format!("删除数据成功：{}", id)),
        ),
        Err(err) => handle_pasty_error(err),
    }
}

#[catch(404)]
fn not_found() -> &'static str {
    "此链接不存在。如果你正在更新链接，可能是漏了参数！"
}

#[catch(500)]
fn internal_error() -> &'static str {
    "服务器内部出错"
}

#[rocket::main]
async fn main() {
    let rocket_instance = rocket::build();
    let figment = rocket_instance.figment();
    let config: Config = figment
        .extract_inner("pasty")
        .expect("error loading configuration");

    let db = Database::create(config.db_path.clone()).expect("error opening database");

    service::ensure_table_exists(&db).expect("error ensuring table exists");

    let result = rocket_instance
        .manage(db)
        .manage(config)
        .register("/", catchers![not_found, internal_error])
        .mount(
            "/",
            routes![
                get_index,
                get_all,
                post_index,
                get_by_id,
                get_stat_by_id,
                post_by_id,
                delete_by_id
            ],
        )
        .launch()
        .await;

    result.expect("error shutting down http server");
}
