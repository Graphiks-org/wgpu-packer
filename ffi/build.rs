use std::env;
use std::path::PathBuf;
use std::process::Command;

fn sdk_path(sdk_name: &str) -> String {
    let output = Command::new("xcrun")
        .args(["--sdk", sdk_name, "--show-sdk-path"])
        .output()
        .expect("xcrun failed")
        .stdout;
    std::str::from_utf8(&output)
        .expect("invalid output from `xcrun`")
        .trim()
        .to_owned()
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    for header in ["webgpu.h", "wgpu.h"] {
        println!("cargo:rerun-if-changed={}", manifest_dir.join("ffi").join(header).display());
    }
    println!("cargo:rerun-if-env-changed=TARGET");
    println!("cargo:rerun-if-env-changed=BINDGEN_EXTRA_CLANG_ARGS");

    // wgpuGetVersion reads WGPU_NATIVE_VERSION at compile time. The single
    // source of truth is wgpu-pin.env (WGPU_VERSION.PACKER_VERSION, packed
    // MSB-first by logging.rs): the version tests and the release manifests
    // read the same values.
    let pin_path = manifest_dir.join("..").join("wgpu-pin.env");
    println!("cargo:rerun-if-changed={}", pin_path.display());
    let pin = std::fs::read_to_string(&pin_path)
        .unwrap_or_else(|e| panic!("reading wgpu-pin.env: {e}"));
    let mut wgpu_version = String::new();
    let mut packer_version = String::new();
    for line in pin.lines() {
        if let Some((key, value)) = line.split_once('=') {
            match key.trim() {
                "WGPU_VERSION" => wgpu_version = value.trim().to_owned(),
                "PACKER_VERSION" => packer_version = value.trim().to_owned(),
                _ => {}
            }
        }
    }
    assert!(
        !wgpu_version.is_empty() && !packer_version.is_empty(),
        "wgpu-pin.env must define WGPU_VERSION and PACKER_VERSION"
    );
    println!("cargo:rustc-env=WGPU_NATIVE_VERSION={wgpu_version}.{packer_version}");

    #[rustfmt::skip]
    let types_to_rename = vec![
        ("WGPUAdapter", "WGPUAdapterImpl"),
        ("WGPUBindGroup", "WGPUBindGroupImpl"),
        ("WGPUBindGroupLayout", "WGPUBindGroupLayoutImpl"),
        ("WGPUBuffer", "WGPUBufferImpl"),
        ("WGPUCommandBuffer", "WGPUCommandBufferImpl"),
        ("WGPUCommandEncoder", "WGPUCommandEncoderImpl"),
        ("WGPUComputePassEncoder", "WGPUComputePassEncoderImpl"),
        ("WGPUComputePipeline", "WGPUComputePipelineImpl"),
        ("WGPUDevice", "WGPUDeviceImpl"),
        ("WGPUInstance", "WGPUInstanceImpl"),
        ("WGPUPipelineLayout", "WGPUPipelineLayoutImpl"),
        ("WGPUExternalTexture", "WGPUExternalTextureImpl"),
        ("WGPUBlas", "WGPUBlasImpl"),
        ("WGPUTlas", "WGPUTlasImpl"),
        ("WGPUPipelineCache", "WGPUPipelineCacheImpl"),
        ("WGPUQuerySet", "WGPUQuerySetImpl"),
        ("WGPUQueue", "WGPUQueueImpl"),
        ("WGPURenderBundle", "WGPURenderBundleImpl"),
        ("WGPURenderBundleEncoder", "WGPURenderBundleEncoderImpl"),
        ("WGPURenderPassEncoder", "WGPURenderPassEncoderImpl"),
        ("WGPURenderPipeline", "WGPURenderPipelineImpl"),
        ("WGPUSampler", "WGPUSamplerImpl"),
        ("WGPUShaderModule", "WGPUShaderModuleImpl"),
        ("WGPUSurface", "WGPUSurfaceImpl"),
        ("WGPUTexture", "WGPUTextureImpl"),
        ("WGPUTextureView", "WGPUTextureViewImpl"),
    ];
    let mut builder = bindgen::Builder::default()
        .header("ffi/wgpu.h")
        .clang_arg(format!("-I{}", manifest_dir.join("ffi").display()))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .allowlist_item("WGPU.*")
        .allowlist_item("wgpu.*")
        .blocklist_function("wgpuGetProcAddress")
        .prepend_enum_name(false)
        .size_t_is_usize(true)
        .ignore_functions()
        .layout_tests(true)
        .clang_macro_fallback();

    for (old_name, new_name) in types_to_rename {
        let line = format!("pub type {old_name} = *const crate::{new_name};");
        builder = builder
            .blocklist_type(old_name)
            .blocklist_type(format!("{old_name}Impl"))
            .raw_line(line);
    }

    if let Ok(target) = env::var("TARGET") {
        match target.as_str() {
            "aarch64-apple-ios" => {
                builder = builder
                    .clang_arg("-isysroot")
                    .clang_arg(sdk_path("iphoneos"))
                    .clang_arg("--target=arm64-apple-ios");
            }
            "aarch64-apple-ios-sim" => {
                builder = builder
                    .clang_arg("-isysroot")
                    .clang_arg(sdk_path("iphonesimulator"))
                    .clang_arg("--target=arm64-apple-ios-simulator");
            }
            "x86_64-apple-ios" => {
                builder = builder
                    .clang_arg("-isysroot")
                    .clang_arg(sdk_path("iphonesimulator"))
                    .clang_arg("--target=x86_64-apple-ios-simulator");
            }
            "aarch64-apple-darwin" | "x86_64-apple-darwin" => {
                builder = builder.clang_arg("-isysroot").clang_arg(sdk_path("macosx"));
            }
            // Android: bindgen needs the NDK sysroot to resolve the C library
            // headers webgpu.h pulls in. NDK_HOME is set by scripts/build-target.sh
            // (and by CI); without it bindgen cannot parse the header at all.
            t if t.ends_with("-linux-android") => {
                if let Ok(ndk) = env::var("NDK_HOME") {
                    let host = match env::consts::OS {
                        "macos" => "darwin-x86_64",
                        _ => "linux-x86_64",
                    };
                    let prebuilt = PathBuf::from(&ndk)
                        .join("toolchains")
                        .join("llvm")
                        .join("prebuilt")
                        .join(host);
                    let sysroot = prebuilt.join("sysroot");
                    if sysroot.is_dir() {
                        // The NDK's cdefs.h rejects an unversioned triple
                        // ("Unversioned target triples are not supported!"),
                        // so append the API level from the matrix
                        // (deployEnv.ANDROID_API, 24 here).
                        let api = env::var("WGPU_ANDROID_API").unwrap_or_else(|_| "24".into());
                        let arch = t.split('-').next().unwrap_or("aarch64");
                        builder = builder
                            .clang_arg(format!("--target={arch}-linux-android{api}"))
                            .clang_arg(format!("--sysroot={}", sysroot.display()));
                    }
                }
            }
            _ => {}
        }
    }

    let bindings = builder.generate().expect("Unable to generate bindings");
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    write_proc_table(&out_path, &manifest_dir);
    write_msvc_exports(&out_path, &manifest_dir);
}

/// MSVC does not export `#[no_mangle]` symbols from a cdylib on its own: the
/// linker needs a module-definition file (or one `/EXPORT:` per symbol).
/// Generate it from the same declaration scan the proc table uses, so the DLL
/// exports exactly what the headers declare.
fn write_msvc_exports(_out_path: &PathBuf, manifest_dir: &PathBuf) {
    // Only MSVC needs this; lld on Linux/macOS exports `#[no_mangle]` symbols
    // on its own. `CARGO_CFG_TARGET_ENV` is empty in a build script, so the
    // target triple identifies MSVC.
    if !env::var("TARGET").unwrap_or_default().ends_with("-msvc") {
        return;
    }
    let mut names: Vec<String> = Vec::new();
    for header in ["webgpu.h", "wgpu.h"] {
        let text = std::fs::read_to_string(manifest_dir.join("ffi").join(header))
            .unwrap_or_else(|e| panic!("reading {header}: {e}"));
        for name in declared_procs(&text) {
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    // The debug-only probe is compiled out of release builds (see
    // ffi_utils.rs); exporting it there would be an unresolved symbol.
    // `CARGO_CFG_DEBUG_ASSERTIONS` is empty in a build script, PROFILE is not.
    if env::var("PROFILE").as_deref() == Ok("release") {
        names.retain(|name| name != "wgpuDebugTriggerPanic");
    }
    names.sort();
    // One `/EXPORT:` per declared symbol. A `.def` file is the other route,
    // but as a linker input it also reaches the test binaries and corrupts
    // them (LNK1107); `rustc-link-arg-cdylib` keeps it on the DLL.
    for name in &names {
        println!("cargo:rustc-link-arg-cdylib=/EXPORT:{name}");
    }
}

/// Resolution table for `wgpuGetProcAddress`: every function declared in the
/// headers (same grammar as tests/checks.py) is resolvable by name. The file
/// is included at the crate root by src/lib.rs.
fn write_proc_table(out_path: &PathBuf, manifest_dir: &PathBuf) {
    let mut procs: Vec<String> = Vec::new();
    for header in ["webgpu.h", "wgpu.h"] {
        let text = std::fs::read_to_string(manifest_dir.join("ffi").join(header))
            .unwrap_or_else(|e| panic!("reading {header}: {e}"));
        for name in declared_procs(&text) {
            if !procs.contains(&name) {
                procs.push(name);
            }
        }
    }

    let mut table = String::from(
        "// @generated by build.rs - do not edit.\n\
         // Source: ffi/ffi/{webgpu,wgpu}.h (grammar of tests/checks.py).\n\n\
         pub fn lookup_proc(name: &str) -> crate::native::WGPUProc {\n    match name {\n",
    );
    for name in &procs {
        // Debug-only probe: the symbol does not exist in release builds
        // (see ffi_utils.rs), so the arm must disappear with it.
        let cfg = if name == "wgpuDebugTriggerPanic" {
            "        #[cfg(debug_assertions)]\n"
        } else {
            ""
        };
        table.push_str(&format!(
            "{cfg}        \"{name}\" => Some(unsafe {{ std::mem::transmute::<*const (), unsafe extern \"C\" fn()>({name} as usize as *const ()) }}),\n"
        ));
    }
    table.push_str("        _ => None,\n    }\n}\n");
    std::fs::write(out_path.join("procs.rs"), table).expect("writing procs.rs");
}

/// Rust translation of the DECL grammar in tests/checks.py: a line declares a
/// function if it starts with `[A-Za-z_]` followed by `[\w \t\*&]*`, does not
/// start with a comment, does not contain "static inline", and names a
/// function `wgpuXxx(`.
fn declared_procs(header: &str) -> Vec<String> {
    let mut out = Vec::new();
    'line: for line in header.lines() {
        let t = line.trim_start();
        if t.starts_with("//") || t.starts_with('*') || t.contains("static inline") {
            continue;
        }
        let bytes = t.as_bytes();
        let Some(&first) = bytes.first() else {
            continue;
        };
        if !(first.is_ascii_alphabetic() || first == b'_') {
            continue;
        }
        let mut i = 1;
        while i + 4 < bytes.len() {
            // A character outside the allowed prefix ends the search on this
            // line: the DECL regex can no longer match there.
            if !(bytes[i].is_ascii_alphanumeric()
                || matches!(bytes[i], b'_' | b' ' | b'\t' | b'*' | b'&'))
            {
                continue 'line;
            }
            if &bytes[i..i + 4] == b"wgpu"
                && bytes[i + 4].is_ascii_uppercase()
                && !bytes[i - 1].is_ascii_alphanumeric()
                && bytes[i - 1] != b'_'
            {
                let mut j = i + 4;
                while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                    j += 1;
                }
                if t[j..].trim_start().starts_with('(') {
                    out.push(t[i..j].to_string());
                }
                continue 'line;
            }
            i += 1;
        }
    }
    out
}
