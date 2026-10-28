fn main() {
    // Optional engines are picked from tools/manifest.json by target triple.
    println!("cargo:rustc-env=MUTABOR_TARGET={}", std::env::var("TARGET").unwrap());
    println!("cargo:rerun-if-changed=../tools/manifest.json");
    tauri_build::build()
}
