use std::fs;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::api::Api;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Favorite {
    pub name: String,
    pub path: String,
    pub provider: FavoriteProvider,
    #[serde(default)]
    pub connection_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "server_url", rename_all = "snake_case")]
pub(crate) enum FavoriteProvider {
    Local,
    Remote(String),
}

impl FavoriteProvider {
    pub fn for_api(api: &Api) -> Self {
        match api.server_url() {
            Some(url) => Self::Remote(url.to_string()),
            None => Self::Local,
        }
    }
}

#[derive(Default)]
pub(crate) struct Favorites {
    pub entries: Vec<Favorite>,
}

impl Favorites {
    pub fn load() -> Self {
        let entries = fs::read(crate::persist::FAVORITES.path())
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Vec<Favorite>>(&bytes).ok())
            .unwrap_or_default();
        Self { entries }
    }

    pub fn add(&mut self, favorite: Favorite) {
        if let Some(existing) = self.entries.iter_mut().find(|existing| {
            existing.path == favorite.path && existing.provider == favorite.provider
        }) {
            existing.name = favorite.name;
            existing.connection_id = favorite.connection_id;
        } else {
            self.entries.push(favorite);
        }
    }

    pub fn remove(&mut self, favorite: &Favorite) {
        self.entries.retain(|existing| {
            existing.path != favorite.path || existing.provider != favorite.provider
        });
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(&self.entries)?)
    }
}

