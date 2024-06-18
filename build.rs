extern crate bindgen;
use fs_extra::dir::CopyOptions;
use glob::glob;
use std::env;
use std::path::{Path, PathBuf};

static SOURCE_DIRECTORY: &str = "libpg_query";
static LIBRARY_NAME: &str = "pg_query";

// https://github.com/pganalyze/pg_query.rs/blob/5562e4aeea885ef514134dcb084d98d6993c8c3a/build.rs
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let build_path = Path::new(".").join("c_libs").join(SOURCE_DIRECTORY);
    let target = env::var("TARGET").unwrap();
    let system_path = env::var("LIBPG_QUERY_PATH").map(PathBuf::from);

    println!("cargo:rustc-link-lib=static=pg_query");

    let out_header_path = if let Ok(system_path) = system_path {
        println!(
            "cargo:rustc-link-search=native={}",
            system_path.join("lib").display()
        );
        system_path.join("include/pg_query.h")
    } else {
        println!("cargo:rustc-link-search=native={}", out_dir.display());
        // Copy the relevant source files to the OUT_DIR
        let source_paths = vec![
            build_path.join("pg_query").with_extension("h"),
            build_path.join("src"),
            build_path.join("protobuf"),
            build_path.join("vendor"),
        ];

        let copy_options = CopyOptions {
            overwrite: true,
            ..CopyOptions::default()
        };

        fs_extra::copy_items(&source_paths, &out_dir, &copy_options)?;

        // Compile the C library.
        let mut build = cc::Build::new();
        build
            .files(
                glob(out_dir.join("src/*.c").to_str().unwrap())
                    .unwrap()
                    .map(|p| p.unwrap()),
            )
            .files(
                glob(out_dir.join("src/postgres/*.c").to_str().unwrap())
                    .unwrap()
                    .map(|p| p.unwrap()),
            )
            .file(out_dir.join("vendor/protobuf-c/protobuf-c.c"))
            .file(out_dir.join("vendor/xxhash/xxhash.c"))
            .file(out_dir.join("protobuf/pg_query.pb-c.c"))
            .include(out_dir.join("."))
            .include(out_dir.join("./vendor"))
            .include(out_dir.join("./src/postgres/include"))
            .include(out_dir.join("./src/include"))
            .warnings(false); // Avoid unnecessary warnings, as they are already considered as part of libpg_query development
        if env::var("PROFILE").unwrap() == "debug" || env::var("DEBUG").unwrap() == "1" {
            build.define("USE_ASSERT_CHECKING", None);
        }
        if target.contains("windows") {
            build.include(out_dir.join("./src/postgres/include/port/win32"));
            if target.contains("msvc") {
                build.include(out_dir.join("./src/postgres/include/port/win32_msvc"));
            }
        }
        build.compile(LIBRARY_NAME);

        out_dir.join(LIBRARY_NAME).with_extension("h")
    };

    // Generate bindings for Rust
    bindgen::Builder::default()
        .header(out_header_path.to_str().ok_or("Invalid header path")?)
        .generate()
        .map_err(|_| "Unable to generate bindings")?
        .write_to_file(out_dir.join("bindings.rs"))?;
    Ok(())
}
