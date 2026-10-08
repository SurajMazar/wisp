//! Embeds `database/migrations/*.sql` into the binary, in filename order.
// Build scripts should fail loudly on a malformed migrations directory.
#![allow(clippy::expect_used)]

use std::{env, fs, path::PathBuf};

fn main() {
    let dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"))
        .join("../../database/migrations");
    println!("cargo:rerun-if-changed={}", dir.display());

    let mut entries: Vec<_> = fs::read_dir(&dir)
        .expect("database/migrations must exist")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "sql"))
        .collect();
    entries.sort();

    let mut out = String::from("pub(crate) static MIGRATIONS: &[(u32, &str, &str)] = &[\n");
    for path in entries {
        println!("cargo:rerun-if-changed={}", path.display());
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("utf-8 file name");
        let (num, name) = stem
            .split_once('_')
            .expect("migration files are NNNN_name.sql");
        let version: u32 = num.parse().expect("migration prefix must be numeric");
        out.push_str(&format!(
            "    ({version}, {name:?}, include_str!({path:?})),\n",
            path = path
                .canonicalize()
                .expect("canonical path")
                .display()
                .to_string()
        ));
    }
    out.push_str("];\n");
    let dest = PathBuf::from(env::var("OUT_DIR").expect("out dir")).join("migrations.rs");
    fs::write(dest, out).expect("write migrations.rs");
}
