//! Discovers migration files and repository modules by scanning directories, so adding a table adds files and edits no shared
//! list (SHARED_SURFACE_PROTOCOL.md section 3). A duplicate migration number is not a build error: it is caught by the
//! `migration_numbers_are_unique` test so the failure names the offending files.

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("manifest dir");
    let out = env::var("OUT_DIR").expect("out dir");
    println!("cargo:rerun-if-changed=migrations");
    println!("cargo:rerun-if-changed=src/repositories");

    let mut migrations: Vec<(u32, String, String)> = Vec::new();
    for entry in fs::read_dir(Path::new(&manifest).join("migrations")).expect("migrations dir") {
        let entry = entry.expect("dir entry");
        let name = entry.file_name().into_string().expect("utf-8 file name");
        if !name.ends_with(".sql") {
            continue;
        }
        let number: u32 = name
            .split('_')
            .next()
            .and_then(|n| n.parse().ok())
            .unwrap_or_else(|| {
                panic!("migration `{name}` must start with a number and an underscore")
            });
        let path = entry.path().to_string_lossy().replace('\\', "/");
        migrations.push((number, name, path));
    }
    migrations.sort();
    let mut code = String::from("pub const MIGRATIONS: &[(u32, &str, &str)] = &[\n");
    for (number, name, path) in &migrations {
        code.push_str(&format!(
            "    ({number}, \"{name}\", include_str!(\"{path}\")),\n"
        ));
    }
    code.push_str("];\n");
    fs::write(Path::new(&out).join("migrations.rs"), code).expect("write migrations.rs");

    let mut repos: Vec<(String, String)> = Vec::new();
    for entry in fs::read_dir(Path::new(&manifest).join("src").join("repositories"))
        .expect("repositories dir")
    {
        let entry = entry.expect("dir entry");
        let name = entry.file_name().into_string().expect("utf-8 file name");
        if name == "mod.rs" || !name.ends_with(".rs") {
            continue;
        }
        let module = name.trim_end_matches(".rs").to_owned();
        repos.push((module, entry.path().to_string_lossy().replace('\\', "/")));
    }
    repos.sort();
    let mut code = String::new();
    for (module, path) in &repos {
        code.push_str(&format!("#[path = \"{path}\"]\npub mod {module};\n"));
    }
    fs::write(Path::new(&out).join("repositories.rs"), code).expect("write repositories.rs");
}
