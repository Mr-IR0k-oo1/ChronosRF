use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-check-cfg=cfg(has_liquid_dsp)");

    for key in [
        "CARGO_FEATURE_NATIVE_DSP",
        "LIQUID_DSP_HEADER",
        "LIQUID_DSP_INCLUDE_DIR",
        "LIQUID_DSP_LIB_DIR",
        "LIQUID_DSP_LIB_NAME",
        "LIQUID_DSP_LINK_KIND",
        "INCLUDE",
        "LIB",
        "MSYSTEM_PREFIX",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
    }

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is required"));
    let bindings_path = out_dir.join("liquid_dsp_bindings.rs");

    if env::var_os("CARGO_FEATURE_NATIVE_DSP").is_none() {
        write_stub_bindings(&bindings_path).expect("failed to write liquid-dsp stub bindings");
        return;
    }

    match generate_bindings(&bindings_path) {
        Ok(true) => {
            println!("cargo:rustc-cfg=has_liquid_dsp");
        }
        Ok(false) => {
            println!(
                "cargo:warning=liquid-dsp headers or libraries were not found; using Rust fallback bindings"
            );
            write_stub_bindings(&bindings_path).expect("failed to write liquid-dsp stub bindings");
        }
        Err(error) => {
            println!("cargo:warning={error}");
            write_stub_bindings(&bindings_path).expect("failed to write liquid-dsp stub bindings");
        }
    }
}

fn generate_bindings(bindings_path: &Path) -> Result<bool, String> {
    let mut include_dirs = collect_dirs("LIQUID_DSP_INCLUDE_DIR");
    include_dirs.extend(collect_dirs("INCLUDE"));

    if let Some(prefix) = env::var_os("MSYSTEM_PREFIX") {
        let prefix = PathBuf::from(prefix);
        include_dirs.push(prefix.join("include"));
    }

    include_dirs.retain(|dir| dir.exists());
    include_dirs.sort();
    include_dirs.dedup();

    let mut lib_dirs = collect_dirs("LIQUID_DSP_LIB_DIR");
    lib_dirs.extend(collect_dirs("LIB"));

    if let Some(prefix) = env::var_os("MSYSTEM_PREFIX") {
        let prefix = PathBuf::from(prefix);
        lib_dirs.push(prefix.join("lib"));
    }

    lib_dirs.retain(|dir| dir.exists());
    lib_dirs.sort();
    lib_dirs.dedup();

    let header = match env::var_os("LIQUID_DSP_HEADER") {
        Some(header_path) => {
            let header_path = PathBuf::from(header_path);
            if header_path.exists() {
                Some(header_path)
            } else {
                None
            }
        }
        None => find_header(&include_dirs),
    };

    let Some(header) = header else {
        return Ok(false);
    };

    if include_dirs.is_empty() || lib_dirs.is_empty() {
        return Ok(false);
    }

    let mut builder = bindgen::Builder::default()
        .header(header.to_string_lossy())
        .allowlist_function("fft_.*")
        .allowlist_type("fftplan")
        .allowlist_var("LIQUID_FFT_.*")
        .generate_comments(false);

    for include_dir in &include_dirs {
        builder = builder.clang_arg(format!("-I{}", include_dir.display()));
    }

    let bindings = builder
        .generate()
        .map_err(|error| format!("bindgen could not generate liquid-dsp bindings: {error}"))?;

    bindings
        .write_to_file(bindings_path)
        .map_err(|error| format!("failed to write liquid-dsp bindings: {error}"))?;

    let link_kind = env::var("LIQUID_DSP_LINK_KIND").unwrap_or_else(|_| "dylib".to_string());
    let library_name = env::var("LIQUID_DSP_LIB_NAME").unwrap_or_else(|_| "liquid".to_string());

    for lib_dir in &lib_dirs {
        println!("cargo:rustc-link-search=native={}", lib_dir.display());
    }

    println!("cargo:rustc-link-lib={}={}", link_kind, library_name);
    Ok(true)
}

fn collect_dirs(key: &str) -> Vec<PathBuf> {
    env::var_os(key)
        .map(|value| env::split_paths(&value).collect())
        .unwrap_or_default()
}

fn find_header(include_dirs: &[PathBuf]) -> Option<PathBuf> {
    include_dirs.iter().find_map(|dir| {
        let direct = dir.join("liquid.h");
        if direct.exists() {
            return Some(direct);
        }

        let nested = dir.join("liquid").join("liquid.h");
        if nested.exists() {
            return Some(nested);
        }

        None
    })
}

fn write_stub_bindings(bindings_path: &Path) -> std::io::Result<()> {
    fs::write(
        bindings_path,
    r#"pub type fftplan = *mut ::libc::c_void;

pub const LIQUID_FFT_FORWARD: i32 = 0;
pub const LIQUID_FFT_BACKWARD: i32 = 1;

pub unsafe fn fft_create_plan(
    _nfft: u32,
    _input: *mut ::libc::c_void,
    _output: *mut ::libc::c_void,
    _direction: i32,
    _method: i32,
) -> fftplan {
    ::core::ptr::null_mut()
}

pub unsafe fn fft_execute(_plan: fftplan) -> i32 {
    0
}

pub unsafe fn fft_destroy_plan(_plan: fftplan) -> i32 {
    0
}
"#,
    )
}