use std::fmt::Display;

use anyhow::bail;
use redb::Database;
use rocket::State;

use crate::*;

const TYPE_TABLE: TableDefinition<&str, u8> = TableDefinition::new("pasty_type");
const STATS_TABLE: TableDefinition<&str, Stats> = TableDefinition::new("pasty_stats");
const CONTENT_TABLE: TableDefinition<&str, String> = TableDefinition::new("pasty_content");
const TOKEN_TABLE: TableDefinition<&str, Option<String>> = TableDefinition::new("pasty_token");

#[derive(Debug, Clone, Copy)]
pub enum PastyError {
    NotFound,
    TokenMismatch,
    TokenRequired,
}

impl Display for PastyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PastyError::NotFound => write!(f, "key not found"),
            PastyError::TokenMismatch => write!(f, "token mismatch"),
            PastyError::TokenRequired => write!(f, "token required"),
        }
    }
}

pub fn ensure_table_exists(db: &Database) -> anyhow::Result<()> {
    let write_tx = db.begin_write()?;

    write_tx.open_table(TYPE_TABLE)?;
    write_tx.open_table(STATS_TABLE)?;
    write_tx.open_table(CONTENT_TABLE)?;
    write_tx.open_table(TOKEN_TABLE)?;

    write_tx.commit()?;

    Ok(())
}

pub fn get_stats_by_id(db: &State<Database>, id: &str) -> anyhow::Result<Stats> {
    let key = stats_key(id);

    let stats = {
        let read_tx = db.begin_read()?;
        let table = read_tx.open_table(STATS_TABLE)?;
        table.get(key.as_str())?
    };

    match stats {
        Some(stats) => Ok(stats.value()),
        None => bail!(PastyError::NotFound),
    }
}

pub fn view_stats_by_id(db: &State<Database>, id: &str) -> anyhow::Result<()> {
    let key = stats_key(id);

    let write_tx = db.begin_write()?;
    {
        let mut table = write_tx.open_table(STATS_TABLE)?;

        let mut stats = match table.get(key.as_str())? {
            Some(stats) => stats.value(),
            None => Stats::new(),
        };

        stats.view();

        table.insert(key.as_str(), stats)?;
    }
    write_tx.commit()?;

    Ok(())
}

fn check_token(
    db: &State<Database>,
    id: &str,
    user_token: Option<&str>,
) -> anyhow::Result<Option<String>> {
    let key = token_key(id);

    let token = match db
        .begin_read()?
        .open_table(TOKEN_TABLE)?
        .get(key.as_str())?
    {
        Some(token) => token.value(),
        None => bail!(PastyError::NotFound),
    };

    if let Some(token) = token.as_ref() {
        if let Some(user_token) = user_token {
            if token != user_token {
                bail!(PastyError::TokenMismatch);
            }
        } else {
            bail!(PastyError::TokenRequired);
        }
    }

    Ok(token)
}

pub fn get_pasty_by_id<'a>(db: &State<Database>, id: &'a str) -> anyhow::Result<Pasty<'a>> {
    let read_tx = db.begin_read()?;

    let key = type_key(id);

    let content_type = match read_tx.open_table(TYPE_TABLE)?.get(key.as_str())? {
        Some(bytes) => ContentType::from(bytes.value()),
        None => bail!(PastyError::NotFound),
    };

    let key = content_key(id);

    let content = match read_tx.open_table(CONTENT_TABLE)?.get(key.as_str())? {
        Some(content) => content.value(),
        None => bail!(PastyError::NotFound),
    };

    Ok(Pasty {
        id,
        content_type,
        content,
    })
}

pub fn update_pasty_by_id(
    db: &State<Database>,
    id: &str,
    content: &str,
    content_type: ContentType,
    user_token: Option<&str>,
) -> anyhow::Result<()> {
    let token_insert = match check_token(db, id, user_token) {
        Ok(_) => false,
        Err(err) => match err.downcast_ref::<PastyError>() {
            Some(PastyError::NotFound) => true,
            _ => return Err(err),
        },
    };

    let write_tx = db.begin_write()?;

    if token_insert {
        write_tx
            .open_table(TOKEN_TABLE)?
            .insert(token_key(id).as_str(), user_token.map(|s| s.to_string()))?;
    }

    let content_type: u8 = content_type.into();

    write_tx
        .open_table(TYPE_TABLE)?
        .insert(type_key(id).as_str(), content_type)?;

    write_tx
        .open_table(CONTENT_TABLE)?
        .insert(content_key(id).as_str(), content.to_string())?;

    let stats = write_tx
        .open_table(STATS_TABLE)?
        .get(stats_key(id).as_str())?
        .map(|stats| stats.value())
        .unwrap_or_else(Stats::new)
        .update();

    write_tx
        .open_table(STATS_TABLE)?
        .insert(stats_key(id).as_str(), stats)?;

    write_tx.commit()?;

    Ok(())
}

pub fn delete_pasty_by_id(
    db: &State<Database>,
    id: &str,
    user_token: Option<&str>,
) -> anyhow::Result<()> {
    check_token(db, id, user_token)?;

    let write_tx = db.begin_write()?;

    write_tx
        .open_table(TYPE_TABLE)?
        .remove(type_key(id).as_str())?;
    write_tx
        .open_table(CONTENT_TABLE)?
        .remove(content_key(id).as_str())?;
    write_tx
        .open_table(TOKEN_TABLE)?
        .remove(token_key(id).as_str())?;
    write_tx
        .open_table(STATS_TABLE)?
        .remove(stats_key(id).as_str())?;

    write_tx.commit()?;

    Ok(())
}
