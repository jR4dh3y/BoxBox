use std::path::Path;

pub fn thumbnail(path: &Path, width: u32, height: u32) -> anyhow::Result<Vec<u8>> {
    Ok(boxbox_thumbnail::thumbnail_path(path, width, height)?)
}
