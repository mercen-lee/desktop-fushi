fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    println!("cargo:rerun-if-changed=assets/desktop-fushi.ico");

    // `winresource` is only declared for Windows builds. Keep the reference behind a host cfg
    // so macOS/Linux builds do not need to compile or download a Windows-only build helper.
    #[cfg(windows)]
    {
        winresource::WindowsResource::new()
            .set_icon("assets/desktop-fushi.ico")
            .set("CompanyName", "Mercen & Rian")
            .set("FileDescription", "Desktop Fushi")
            .set("InternalName", "desktop-fushi")
            .set("OriginalFilename", "Desktop Fushi.exe")
            .set("ProductName", "Desktop Fushi")
            .set("Comments", "https://desktopfushi.mercen.net")
            .set("Homepage", "https://desktopfushi.mercen.net")
            .set("LegalCopyright", "Copyright (c) 2026 Mercen & Rian")
            .compile()
            .expect("failed to embed Windows icon resource");
    }

    #[cfg(not(windows))]
    println!("cargo:warning=skipping Windows icon resource because the build host is not Windows");
}
