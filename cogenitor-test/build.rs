use std::{env, path::Path};

use cogenitor::*;

fn main() {
    let input_path = Path::new("..").join("test-data").join("petstore.yaml");
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let generated_dir = Path::new(&out_dir);

    std::fs::create_dir_all(&generated_dir).unwrap();

    println!("cargo::warning=cwd: {:?}", std::env::current_dir());
    let output_path = generated_dir.join("petstore.rs");
    println!(
        "cargo::warning=output_path: {} ",
        output_path.to_owned().to_string_lossy()
    );
    println!("cargo::warning=input_path={input_path:?}");
    println!("cargo::warning=out_dir={out_dir:?}");
    cogenitor::generate_file(
        &ApiConfig::new_from_path(input_path.as_os_str().to_string_lossy().into_owned()),
        &output_path,
    )
    .unwrap();
}
