fn main() {
    // Icon files are embedded into the Windows resource table by tauri-build.
    // Make sure changing them reruns this build script instead of reusing a
    // stale resource.lib from target/.
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build()
}
