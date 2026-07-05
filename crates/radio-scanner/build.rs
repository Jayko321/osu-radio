use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

const HELPER_NAME: &str = "osu-lazer-realm-parser";

fn main() {
    println!("cargo:rerun-if-env-changed=OSU_LAZER_REALM_PARSER_PATH");

    if let Some(helper_path) = env::var_os("OSU_LAZER_REALM_PARSER_PATH") {
        println!(
            "cargo:rustc-env=OSU_LAZER_REALM_PARSER_BUILT_PATH={}",
            PathBuf::from(helper_path).display()
        );
        return;
    }

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let project_dir = manifest_dir.join("../../tools/osu-lazer-realm-parser");
    let project = project_dir.join(format!("{HELPER_NAME}.csproj"));

    println!("cargo:rerun-if-changed={}", project.display());
    println!(
        "cargo:rerun-if-changed={}",
        project_dir.join("Program.cs").display()
    );

    let output_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("dotnet-helper");
    let dotnet_home = env::var_os("DOTNET_CLI_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("../../target/dotnet-home"));
    let status = Command::new("dotnet")
        .args(["build", "--configuration", "Release", "--nologo", "--output"])
        .arg(&output_dir)
        .arg(&project)
        .env("DOTNET_CLI_HOME", dotnet_home)
        .env("DOTNET_NOLOGO", "1")
        .env("DOTNET_SKIP_FIRST_TIME_EXPERIENCE", "1")
        .status()
        .unwrap_or_else(|error| {
            panic!(
                "failed to start dotnet while building {}: {error}. Install the .NET 8 SDK or set OSU_LAZER_REALM_PARSER_PATH to a prebuilt helper",
                project.display()
            )
        });

    if !status.success() {
        panic!("dotnet failed to build {} ({status})", project.display());
    }

    let helper_path = output_dir.join(executable_name(HELPER_NAME));
    if !helper_path.is_file() {
        panic!(
            "dotnet reported success but did not produce {}",
            helper_path.display()
        );
    }

    println!(
        "cargo:rustc-env=OSU_LAZER_REALM_PARSER_BUILT_PATH={}",
        helper_path.display()
    );
}

fn executable_name(name: &str) -> PathBuf {
    if cfg!(windows) {
        Path::new(&format!("{name}.exe")).to_path_buf()
    } else {
        Path::new(name).to_path_buf()
    }
}
