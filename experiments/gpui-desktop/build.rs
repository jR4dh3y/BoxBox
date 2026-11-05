//! Embeds every extracted Lucide icon, so `scripts/extract-icons.ts` is the only list to edit.

use std::fmt::Write;

fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icons");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("assets/icons")
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter_map(|name| name.strip_suffix(".svg").map(String::from))
        .collect();
    names.sort();
    let mut out = String::from("const ICONS: &[(&str, &[u8])] = &[\n");
    for name in names {
        for variant in ["", "thin/"] {
            let file = dir.join(format!("{variant}{name}.svg"));
            writeln!(out, "    (\"icons/{variant}{name}.svg\", include_bytes!({:?})),", file.display().to_string()).unwrap();
        }
    }
    out.push_str("];\n");
    let target = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("icons.rs");
    std::fs::write(target, out).unwrap();
}
