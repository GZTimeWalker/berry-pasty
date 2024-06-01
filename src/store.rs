use chrono::{DateTime, Utc};

pub struct Pasty<'a> {
    pub id: &'a str,
    pub content_type: ContentType,
    pub content: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ContentType {
    Plaintext = 0,
    Redirect = 1,
}

impl From<ContentType> for u8 {
    fn from(val: ContentType) -> Self {
        val as u8
    }
}

impl From<u8> for ContentType {
    fn from(val: u8) -> Self {
        match val {
            0 => ContentType::Plaintext,
            1 => ContentType::Redirect,
            _ => panic!("invalid content type value: {}", val),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Stats {
    pub views: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_viewed_at: DateTime<Utc>,
}

impl Stats {
    const SIZE: usize = 4 + 8 * 3;

    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            views: 0,
            created_at: now,
            updated_at: now,
            last_viewed_at: now,
        }
    }

    pub fn view(&mut self) {
        self.views += 1;
        self.last_viewed_at = Utc::now();
    }

    pub fn update(mut self) -> Self {
        self.updated_at = Utc::now();
        self
    }
}

impl redb::Value for Stats {
    type SelfType<'a> = Stats
    where
        Self: 'a;

    type AsBytes<'a> = [u8; Self::SIZE]
    where
        Self: 'a;

    fn fixed_width() -> Option<usize> {
        Some(Self::SIZE)
    }

    fn from_bytes<'a>(bytes: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        let views = u32::from_be_bytes(bytes[..4].try_into().unwrap());
        let created_ts = i64::from_be_bytes(bytes[4..12].try_into().unwrap());
        let created_at = DateTime::from_timestamp(created_ts, 0).unwrap_or_default();
        let updated_ts = i64::from_be_bytes(bytes[12..20].try_into().unwrap());
        let updated_at = DateTime::from_timestamp(updated_ts, 0).unwrap_or_default();
        let last_viewed_ts = i64::from_be_bytes(bytes[20..28].try_into().unwrap());
        let last_viewed_at = DateTime::from_timestamp(last_viewed_ts, 0).unwrap_or_default();
        Self {
            views,
            created_at,
            updated_at,
            last_viewed_at,
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'a,
        Self: 'b,
    {
        let mut bytes = [0; Self::SIZE];
        bytes[..4].copy_from_slice(&value.views.to_be_bytes());
        bytes[4..12].copy_from_slice(&value.created_at.timestamp().to_be_bytes());
        bytes[12..20].copy_from_slice(&value.updated_at.timestamp().to_be_bytes());
        bytes[20..28].copy_from_slice(&value.last_viewed_at.timestamp().to_be_bytes());
        bytes
    }

    fn type_name() -> redb::TypeName {
        TypeName::new("stats")
    }
}

pub enum KeyType {
    Type,
    Content,
    Token,
    Stats,
}

pub const fn key_type_str(t: KeyType) -> &'static str {
    match t {
        KeyType::Type => "t",
        KeyType::Content => "ct",
        KeyType::Token => "tk",
        KeyType::Stats => "st",
    }
}

use paste::paste;
use redb::TypeName;

macro_rules! key_fn {
    ($key_type:ident) => {
        paste! {
            #[inline]
            pub fn [<$key_type:lower _key>](id: &str) -> String {
                format!("{}:{}", id, key_type_str(KeyType::$key_type))
            }
        }
    };
}

key_fn!(Type);
key_fn!(Content);
key_fn!(Token);
key_fn!(Stats);
