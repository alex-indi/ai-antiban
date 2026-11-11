fn main() {
    embed_test_manifest_on_windows();
    tauri_build::build()
}

/// Windows (MSVC): test executables (lib unit tests and `tests/*.rs`) do not get the
/// application manifest that tauri-build links only into the app binary. Without
/// Common Controls v6 the test exe dies at startup with 0xc0000139
/// (STATUS_ENTRYPOINT_NOT_FOUND, comctl32!TaskDialogIndirect), see tauri-apps/tauri#13419.
///
/// `rustc-link-arg-tests` does not cover the lib unit-test binary, so the manifest is
/// passed to every target and then switched off again for the app binary and the cdylib,
/// which keeps the release manifest exactly what tauri-build generates (later /MANIFEST
/// options win, and cargo keeps the order in which they are printed here).
fn embed_test_manifest_on_windows() {
    let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("windows")
        .join("test.manifest");
    println!("cargo:rerun-if-changed={}", manifest.display());

    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        return;
    }
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    println!("cargo:rustc-link-arg-bins=/MANIFEST:NO");
    println!("cargo:rustc-link-arg-cdylib=/MANIFEST:NO");
}
