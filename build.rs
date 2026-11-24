fn main() {
    #[cfg(feature = "shroudtopia")]
    {
        assert_eq!(std::env::var("TARGET").as_deref(), Ok("x86_64-pc-windows-msvc"),
            "the Shroudtopia adapter requires the Windows x64 MSVC ABI");
        println!("cargo:rerun-if-changed=src/shroudtopia.cpp");
        cc::Build::new().cpp(true).std("c++17").static_crt(true)
            .define("GUARDIAN_VERSION", Some(concat!("\"", env!("CARGO_PKG_VERSION"), "\"")))
            .file("src/shroudtopia.cpp").compile("guardian_shroudtopia");
    }
}
