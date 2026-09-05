fn main() {
    #[cfg(feature = "desktop")]
    {
        println!("cargo:rerun-if-changed=icons");
        tauri_build::build();
    }
}
