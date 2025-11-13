use std::{env, path::Path};

use cogenitor::*;

fn main() {
    if let Err(e) = build() {
        println!("cargo::error={e}");
    }
}

fn build() -> anyhow::Result<()> {
    let input_path = Path::new(&std::env::current_dir()?)
        .join("..")
        .join("test-data")
        .join("petstore.yaml");
    let out_dir = env::var_os("OUT_DIR").ok_or(anyhow::format_err!(
        "OUT_DIR environment variable not defined"
    ))?;
    let generated_dir = Path::new(&out_dir);

    std::fs::create_dir_all(&generated_dir)?;

    // NOTE: I currently see no other way than printing warnings to make cargo print
    // this info out on every build, regardless of success or failure
    println!("cargo::warning=input_path={input_path:?}");
    let output_path = generated_dir.join("petstore.rs");
    println!(
        "cargo::warning=output_path: {} ",
        output_path.to_owned().to_string_lossy()
    );
    cogenitor::generate_file(
        &ApiConfig::new_from_path(input_path.as_os_str().to_string_lossy().into_owned()),
        &output_path,
    )?;

    Ok(())
}
