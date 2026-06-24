//! Build script: compile protobuf definitions and generate the plugin
//! C-ABI header. Both steps are native-only (skipped on wasm32).

/// Write generated bindings to `<crate_dir>/include/<name>`, creating the
/// `include/` dir if needed.
fn write_header(
    crate_dir: &std::path::Path,
    name: &str,
    bindings: &cbindgen::Bindings,
) -> Result<(), Box<dyn std::error::Error>> {
    let output_file = crate_dir.join("include").join(name);
    if let Some(parent) = output_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let _wrote = bindings.write_to_file(&output_file);
    Ok(())
}

/// Run cbindgen with the abi-scoped config and write the plugin C-ABI header
/// into `include/foldit_plugin_abi.h`. Single-sources the header from
/// `src/abi.rs`, so the C and Rust sides cannot drift.
fn generate_abi_header() -> Result<(), Box<dyn std::error::Error>> {
    let crate_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let config = cbindgen::Config::from_file(crate_dir.join("cbindgen-abi.toml"))
        .map_err(|e| format!("cbindgen-abi.toml: {e}"))?;
    let bindings = cbindgen::generate_with_config(&crate_dir, config)?;
    write_header(&crate_dir, "foldit_plugin_abi.h", &bindings)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Compile protobuf messages. protox is a pure-Rust protobuf compiler, so
    // no system protoc is required on the build host. Generated code is
    // no_std-friendly so it compiles on every target.
    let fds = protox::compile(["proto/plugin.proto"], ["proto"])?;
    prost_build::compile_fds(fds)?;

    // On wasm32: no native C consumers, so skip cbindgen output.
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        println!("cargo:rerun-if-changed=proto/plugin.proto");
        return Ok(());
    }

    generate_abi_header()?;

    println!("cargo:rerun-if-changed=proto/plugin.proto");
    println!("cargo:rerun-if-changed=cbindgen-abi.toml");
    println!("cargo:rerun-if-changed=cbindgen.toml");
    println!("cargo:rerun-if-changed=src/abi.rs");
    println!("cargo:rerun-if-changed=include/foldit_plugin_abi.h");

    Ok(())
}
