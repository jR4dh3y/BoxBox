use std::{fs, path::PathBuf};

use anyhow::{Context as _, Result};
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
        let entries = fs::read(path())
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

    pub fn save(&self) -> Result<()> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

        let path = path();
        let parent = path.parent().context("favorites path has no parent")?;
        fs::create_dir_all(parent).with_context(|| format!("cannot create {}", parent.display()))?;
        let bytes = serde_json::to_vec_pretty(&self.entries)?;
        let temporary = path.with_file_name(format!(
            "favorites.json.{}.{}.tmp",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        if let Err(error) = fs::write(&temporary, bytes) {
            let _ = fs::remove_file(&temporary);
            return Err(error).with_context(|| format!("cannot write {}", temporary.display()));
        }
        if let Err(error) = fs::rename(&temporary, &path) {
            let _ = fs::remove_file(&temporary);
            return Err(error).with_context(|| format!("cannot replace {}", path.display()));
        }
        Ok(())
    }
}

fn path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("boxbox-gpui")
        .join("favorites.json")
}
