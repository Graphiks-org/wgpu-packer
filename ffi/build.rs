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
    for header in ["webgpu.h", "wgpu_upstream.h", "wgpu.h"] {
        println!("cargo:rerun-if-changed={}", manifest_dir.join("ffi").join(header).display());
    }
    println!("cargo:rerun-if-env-changed=TARGET");
    println!("cargo:rerun-if-env-changed=BINDGEN_EXTRA_CLANG_ARGS");

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
            _ => {}
        }
    }

    let bindings = builder.generate().expect("Unable to generate bindings");
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    write_proc_table(&out_path, &manifest_dir);
}

/// Table de résolution pour `wgpuGetProcAddress` : chaque fonction déclarée
/// dans les headers (même grammaire que tests/checks.py) est résoluble par
/// nom. Le fichier est inclus à la racine du crate par src/lib.rs.
fn write_proc_table(out_path: &PathBuf, manifest_dir: &PathBuf) {
    let mut procs: Vec<String> = Vec::new();
    for header in ["webgpu.h", "wgpu_upstream.h", "wgpu.h"] {
        let text = std::fs::read_to_string(manifest_dir.join("ffi").join(header))
            .unwrap_or_else(|e| panic!("lecture de {header} : {e}"));
        for name in declared_procs(&text) {
            if !procs.contains(&name) {
                procs.push(name);
            }
        }
    }

    let mut table = String::from(
        "// @generated par build.rs — ne pas éditer.\n\
         // Source : ffi/ffi/{webgpu,wgpu_upstream,wgpu}.h (grammaire de tests/checks.py).\n\n\
         pub fn lookup_proc(name: &str) -> crate::native::WGPUProc {\n    match name {\n",
    );
    for name in &procs {
        // Sonde debug-only : le symbole n'existe pas dans les builds release
        // (cf. ffi_utils.rs), l'arme doit disparaître avec lui.
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

/// Traduction Rust de la grammaire DECL de tests/checks.py : une ligne déclare
/// une fonction si elle commence par `[A-Za-z_]` suivi de `[\w \t\*&]*`, ne
/// démarre pas par un commentaire, ne contient pas « static inline », et nomme
/// une fonction `wgpuXxx(`.
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
            // Un caractère hors du préfixe autorisé clôt la recherche sur la
            // ligne : la regex DECL ne peut plus y apparattre.
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
