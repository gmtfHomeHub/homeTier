fn main() {
    tauri_build::build();

    // iOS-specific build steps
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "ios" {
        println!("cargo:warning=Building for iOS - easytier-ios-staticlib will be compiled");

        // The staticlib is built as a separate crate, cargo will handle it
        // if it's in the workspace. We just need to ensure it's linked.

        // Note: Xcode project injection for NetworkExtension is done in CI
        // via the inject_ne_target.py script, not here in build.rs
        // because build.rs runs in the cargo context, not the Xcode context.
    }

    // Re-run if build.rs changes
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=gen-scripts/ios/");
    println!("cargo:rerun-if-changed=easytier-ios-staticlib/");
}