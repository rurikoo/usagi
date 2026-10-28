use std::{env, fs, path::PathBuf};

fn env_flag(name: &str, default: bool) -> bool {
    println!("cargo:rerun-if-env-changed={name}");

    match env::var(name) {
        Ok(value) => matches!(
            value.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => default,
    }
}

fn main() {
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set"));

    let source_dir = manifest_dir.join("third_party/libjnivm");

    if !source_dir.join("CMakeLists.txt").is_file() {
        panic!(
            "libjnivm submodule is missing. Run: \
             git submodule update --init --recursive"
        );
    }

    println!(
        "cargo:rerun-if-changed={}",
        source_dir.join("CMakeLists.txt").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        source_dir.join("src").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        source_dir.join("include").display()
    );

    let trace = env_flag("USAGI_JNIVM_TRACE", false);
    let debug = env_flag("USAGI_JNIVM_DEBUG", true);

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));

    let lib_dir = out_dir.join("lib");
    fs::create_dir_all(&lib_dir).expect("failed to create native library directory");

    let lib_dir_string = lib_dir.to_string_lossy().into_owned();

    let mut config = cmake::Config::new(&source_dir);

    config
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON")
        .define("CMAKE_ARCHIVE_OUTPUT_DIRECTORY", &lib_dir_string)
        .define("JNIVM_ENABLE_TESTS", "OFF")
        .define("JNIVM_ENABLE_TRACE", if trace { "ON" } else { "OFF" })
        .define("JNIVM_ENABLE_DEBUG", if debug { "ON" } else { "OFF" })
        .define("JNIVM_ENABLE_GC", "ON");

    for profile in ["DEBUG", "RELEASE", "RELWITHDEBINFO", "MINSIZEREL"] {
        let key = format!("CMAKE_ARCHIVE_OUTPUT_DIRECTORY_{profile}");
        config.define(&key, &lib_dir_string);
    }

    config.build_target("jnivm");
    config.build();

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static=jnivm");

    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("CARGO_CFG_TARGET_OS is set by Cargo");
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    match target_os.as_str() {
        "linux" => {
            println!("cargo:rustc-link-lib=dylib=stdc++");
            println!("cargo:rustc-link-lib=dylib=dl");
        }
        "android" => {
            println!("cargo:rustc-link-lib=dylib=c++_shared");
            println!("cargo:rustc-link-lib=dylib=dl");
        }
        "macos" | "ios" => {
            println!("cargo:rustc-link-lib=dylib=c++");
        }
        "windows" if target_env == "gnu" => {
            println!("cargo:rustc-link-lib=dylib=stdc++");
        }
        "windows" => {
            // msvc provides the C++ runtime
        }
        other => {
            panic!("unsupported target OS for libjnivm: {other}");
        }
    }
}
