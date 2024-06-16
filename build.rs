extern crate bindgen;
use fs_extra::dir::CopyOptions;
use std::env;
use std::path::{Path, PathBuf};
use glob::glob;
use std::process::Command;

static SOURCE_DIRECTORY: &str = "libpg_query";
static LIBRARY_NAME: &str = "pg_query";


fn main() -> Result<(), Box<dyn std::error::Error>>  {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let build_path = Path::new(".").join("c_libs").join(SOURCE_DIRECTORY);
    let out_header_path = out_dir.join(LIBRARY_NAME).with_extension("h");
    let target = env::var("TARGET").unwrap();


    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=pg_query");

    // Copy the relevant source files to the OUT_DIR
    let source_paths = vec![
        build_path.join("pg_query").with_extension("h"),
        build_path.join("Makefile"),
        build_path.join("src"),
        build_path.join("protobuf"),
        build_path.join("vendor"),
    ];

    let copy_options = CopyOptions { overwrite: true, ..CopyOptions::default() };

    fs_extra::copy_items(&source_paths, &out_dir, &copy_options)?;


    // Compile the C library.
    let mut build = cc::Build::new();
    build
        .files(glob(out_dir.join("src/*.c").to_str().unwrap()).unwrap().map(|p| p.unwrap()))
        .files(glob(out_dir.join("src/postgres/*.c").to_str().unwrap()).unwrap().map(|p| p.unwrap()))
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

    // Generate bindings for Rust
    bindgen::Builder::default()
        .header(out_header_path.to_str().ok_or("Invalid header path")?)
        .generate()
        .map_err(|_| "Unable to generate bindings")?
        .write_to_file(out_dir.join("bindings.rs"))?;
        Ok(())
}

fn build_from_system(system_path: &Path) {
    println!(
        "cargo:rustc-link-search=native={}",
        system_path.join("lib").display()
    );
}

fn build_from_source(out_dir: &Path) {
    run_command(
        "cp",
        &[
            "-r",
            "./c_libs/libpg_query",
            &out_dir.display().to_string(),
        ],
        None,
    );

    let make_dir = format!("{}/libpg_query", out_dir.display());
    run_command("make", &[], Some(make_dir));

    println!(
        "cargo:rustc-link-search=native={}",
        out_dir.join("libpg_query").display()
    );
}

fn run_command(exe: &str, args: &[&str], dir: Option<String>) {
    let mut c = Command::new(exe);
    c.args(args);
    if let Some(dir) = dir {
        c.current_dir(dir);
    }

    let output = c.output().expect(&format!(
        "failed to run command: {}",
        command_str(exe, args),
    ));
    let code = output.status.code().unwrap_or(-1);
    if code != 0 {
        let mut msg = format!(
            "Failed to run {} with exit code of {}",
            command_str(exe, args),
            code
        );
        if !output.stdout.is_empty() {
            if let Ok(out) = String::from_utf8(output.stdout) {
                msg.push('\n');
                msg.push_str(&format!("stdout =\n{out}"));
            }
        }
        if !output.stderr.is_empty() {
            if let Ok(out) = String::from_utf8(output.stderr) {
                msg.push('\n');
                msg.push_str(&format!("stderr =\n{out}"));
            }
        }
        panic!("{}", msg);
    }
}

fn command_str(exe: &str, args: &[&str]) -> String {
    format!("{exe} {}", args.join(" "))
}
