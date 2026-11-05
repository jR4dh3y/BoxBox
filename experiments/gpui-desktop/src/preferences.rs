//! Desktop preferences (`api/desktopPreferences.ts`), stored by the server as the same JSON file
//! the Electrobun renderer writes. Unknown keys are kept so neither client drops the other's.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SortBy {
    Name,
    Size,
    ModTime,
    Type,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortDirPref {
    Asc,
    Desc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewPref {
    List,
    Grid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Favorite {
    pub name: String,
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Preferences {
    #[serde(rename = "showHiddenFiles")]
    pub show_hidden: bool,
    #[serde(rename = "showFileExtensions")]
    pub show_extensions: bool,
    pub confirm_delete: bool,
    pub default_sort_by: SortBy,
    pub default_sort_dir: SortDirPref,
    pub default_view_mode: ViewPref,
    pub preview_on_single_click: bool,
    pub selection_checkboxes: bool,
    pub compact_mode: bool,
    pub drive_name_overrides: std::collections::BTreeMap<String, String>,
    pub favorite_folders: Vec<Favorite>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            show_hidden: false,
            show_extensions: true,
            confirm_delete: true,
            default_sort_by: SortBy::Name,
            default_sort_dir: SortDirPref::Asc,
            default_view_mode: ViewPref::List,
            preview_on_single_click: false,
            selection_checkboxes: false,
            compact_mode: false,
            drive_name_overrides: Default::default(),
            favorite_folders: Vec::new(),
            extra: Default::default(),
        }
    }
}

impl Preferences {
    pub fn is_favorite(&self, path: &str) -> bool {
        self.favorite_folders
            .iter()
            .any(|favorite| favorite.path == path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_unknown_keys_and_fills_defaults() {
        let saved = r##"{"showHiddenFiles":true,"accentColor":"#ff0000","favoriteFolders":[{"name":"Docs","path":"home/Docs"}]}"##;
        let prefs: Preferences = serde_json::from_str(saved).unwrap();
        assert!(prefs.show_hidden && prefs.confirm_delete);
        assert!(prefs.is_favorite("home/Docs"));
        let round = serde_json::to_value(&prefs).unwrap();
        assert_eq!(round["accentColor"], "#ff0000");
        assert_eq!(round["defaultSortBy"], "name");
    }
}
