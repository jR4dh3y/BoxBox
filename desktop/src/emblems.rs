//! Every Lucide icon, for folder emblems: `assets/emblems.tsv`, written by
//! `scripts/extract-icons.ts`. Each SVG is built at the stroke its use needs and
//! served as `emblems/{stroke}/{name}.svg` (`assets.rs`).

use std::sync::OnceLock;

static TABLE: &str = include_str!("../assets/emblems.tsv");

/// A folder emblem the user can choose.
pub struct Icon {
    pub name: &'static str,
    /// Its other names, for search.
    pub aliases: &'static str,
    body: &'static str,
}

/// The stroke an emblem is drawn with.
#[derive(Clone, Copy)]
pub enum Stroke {
    /// Lucide's own, for a small emblem inside the folder and the picker.
    Regular,
    /// For the corner emblem, a third of the folder: as bold as the outline.
    Bold,
}

impl Stroke {
    fn width(self) -> &'static str {
        match self {
            Self::Regular => "2",
            Self::Bold => "3.6",
        }
    }
}

pub fn all() -> &'static [Icon] {
    static ICONS: OnceLock<Vec<Icon>> = OnceLock::new();
    ICONS.get_or_init(|| {
        let mut icons = TABLE
            .lines()
            .filter_map(|line| {
                let mut fields = line.splitn(3, '\t');
                Some(Icon { name: fields.next()?, aliases: fields.next()?, body: fields.next()? })
            })
            .collect::<Vec<_>>();
        // By name, for `find`'s binary search.
        icons.sort_unstable_by_key(|icon| icon.name);
        icons
    })
}

/// The icon called `name`, by its name or one of its aliases.
pub fn find(name: &str) -> Option<&'static Icon> {
    let icons = all();
    icons
        .binary_search_by(|icon| icon.name.cmp(name))
        .ok()
        .map(|index| &icons[index])
        .or_else(|| icons.iter().find(|icon| icon.aliases.split(' ').any(|alias| alias == name)))
}

/// The icons whose names or aliases hold every word of `query`, in name order.
pub fn search(query: &str) -> Vec<&'static Icon> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    all()
        .iter()
        .filter(|icon| words.iter().all(|word| icon.name.contains(word.as_str()) || icon.aliases.contains(word.as_str())))
        .collect()
}

/// The asset path that draws `name` with `stroke`.
pub fn path(name: &str, stroke: Stroke) -> String {
    format!("emblems/{}/{name}.svg", stroke.width())
}

/// The SVG for an asset path from `path`.
pub fn svg(path: &str) -> Option<Vec<u8>> {
    let (width, file) = path.strip_prefix("emblems/")?.split_once('/')?;
    let width = [Stroke::Regular, Stroke::Bold].into_iter().map(Stroke::width).find(|known| *known == width)?;
    let icons = all();
    let name = file.strip_suffix(".svg")?;
    let index = icons.binary_search_by(|icon| icon.name.cmp(name)).ok()?;
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="{width}" stroke-linecap="round" stroke-linejoin="round">{}</svg>"##,
        icons[index].body
    );
    Some(svg.into_bytes())
}

#[cfg(test)]
mod tests {
    #[test]
    fn icons_are_found_by_name_or_alias_and_searched_by_words() {
        assert!(super::all().len() > 1000);
        assert_eq!(super::find("gamepad-2").map(|icon| icon.name), Some("gamepad-2"));
        assert_eq!(super::find("home").map(|icon| icon.name), Some("house"), "an alias finds its icon");
        let names: Vec<&str> = super::search("Game pad").iter().map(|icon| icon.name).collect();
        assert!(names.contains(&"gamepad-2"), "{names:?}");
        let svg = String::from_utf8(super::svg(&super::path("house", super::Stroke::Bold)).unwrap()).unwrap();
        assert!(svg.contains(r#"stroke-width="3.6""#) && svg.contains("<path"));
        assert!(super::svg("emblems/9/house.svg").is_none());
    }
}
