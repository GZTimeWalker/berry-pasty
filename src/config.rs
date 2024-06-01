use rocket::serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub db_path: String,
    pub random_id_length: u32,
    pub access_password: String,
    pub index_text: String,
    pub index_link: String,
}
