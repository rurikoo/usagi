use std::{env, path::PathBuf};

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

    let jnivm_dir = manifest_dir.join("third_party/libjnivm");

    for path in [
        manifest_dir.join("CMakeLists.txt"),
        manifest_dir.join("cpp/bridge.cpp"),
        manifest_dir.join("include/usagi.h"),
        jnivm_dir.join("CMakeLists.txt"),
    ] {
        if !path.is_file() {
            panic!(
                "required build file is missing: {}\n\
                 check your repository and submodules",
                path.display()
            );
        }

        println!("cargo:rerun-if-changed={}", path.display());
    }

    for path in [jnivm_dir.join("src"), jnivm_dir.join("include")] {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    let trace = env_flag("USAGI_JNIVM_TRACE", false);
    let debug = env_flag("USAGI_JNIVM_DEBUG", true);
    let gc = env_flag("USAGI_JNIVM_GC", true);

    let dst = cmake::Config::new(&manifest_dir)
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("CMAKE_POSITION_INDEPENDENT_CODE", "ON")
        .define("JNIVM_ENABLE_TESTS", "OFF")
        .define("JNIVM_ENABLE_TRACE", if trace { "ON" } else { "OFF" })
        .define("JNIVM_ENABLE_DEBUG", if debug { "ON" } else { "OFF" })
        .define("JNIVM_ENABLE_GC", if gc { "ON" } else { "OFF" })
        .build();

    let lib_dir = dst.join("lib");

    for lib in ["usagi_bridge", "jnivm"] {
        if !lib_dir.join(format!("lib{lib}.a")).is_file()
            && !lib_dir.join(format!("{lib}.lib")).is_file()
        {
            println!(
                "cargo:warning=expected {lib} static library in {}",
                lib_dir.display()
            );
        }
    }

    println!("cargo:rustc-link-search=native={}", lib_dir.display());

    // link the bridge
    println!("cargo:rustc-link-lib=static=usagi_bridge");
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
