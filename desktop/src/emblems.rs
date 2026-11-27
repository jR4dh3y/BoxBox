//! Every Lucide icon, for folder emblems: `assets/emblems.tsv`, written by
//! `scripts/extract-icons.ts`. Each SVG is built with Lucide's stroke when it is
//! drawn and served as `emblems/{name}.svg` (`assets.rs`).

use std::io::Read as _;
use std::sync::OnceLock;

use flate2::read::DeflateDecoder;

static TABLE_COMPRESSED: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/emblems.deflate"));

/// A folder emblem the user can choose.
pub struct Icon {
    pub name: &'static str,
    /// Its other names, for search.
    pub aliases: &'static str,
    body: &'static str,
}

pub fn all() -> &'static [Icon] {
    static ICONS: OnceLock<Vec<Icon>> = OnceLock::new();
    ICONS.get_or_init(|| {
        let mut raw = String::with_capacity(330 * 1024);
        DeflateDecoder::new(TABLE_COMPRESSED).read_to_string(&mut raw).expect("decompress emblems");
        let table: &'static str = Box::leak(raw.into_boxed_str());
        let mut icons = table
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

/// The asset path that draws `name`.
pub fn path(name: &str) -> String {
    format!("emblems/{name}.svg")
}

/// The SVG for an asset path from `path`.
pub fn svg(path: &str) -> Option<Vec<u8>> {
    let name = path.strip_prefix("emblems/")?.strip_suffix(".svg")?;
    let icons = all();
    let index = icons.binary_search_by(|icon| icon.name.cmp(name)).ok()?;
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#000" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{}</svg>"##,
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
        let svg = String::from_utf8(super::svg(&super::path("house")).unwrap()).unwrap();
        assert!(svg.contains(r#"stroke-width="2""#) && svg.contains("<path"));
        assert!(super::svg("emblems/no-such-icon.svg").is_none());
    }
}
