fn main() {
 println!("cargo:rerun-if-changed=icon.ico");
 slint_build::compile("../src/app.slint").unwrap();
 #[cfg(windows)] if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") { winresource::WindowsResource::new().set_icon("icon.ico").compile().unwrap(); }
}
