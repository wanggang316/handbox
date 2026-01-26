fn main() {
    swift_rs::SwiftLinker::new("10.15")
        .with_package("macos-obs", "./macos-obs/")
        .link();
    tauri_build::build()
}
