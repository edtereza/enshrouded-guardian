fn main() {
    #[cfg(feature = "shroudtopia")]
    {
        assert_eq!(
            std::env::var("TARGET").as_deref(),
            Ok("x86_64-pc-windows-msvc"),
            "the Shroudtopia adapter requires the Windows x64 MSVC ABI"
        );
        println!("cargo:rerun-if-changed=src/shroudtopia.cpp");
        println!("cargo:rerun-if-changed=native");
        let native_dir =
            std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("guardian-native");
        let compiler = cc::Build::new().cpp(true).get_compiler();
        let run = |args: &[&std::ffi::OsStr]| {
            let mut command = std::process::Command::new("cmake");
            command.args(args);
            for (key, value) in compiler.get_envs() {
                command.env(key, value);
            }
            assert!(
                command
                    .status()
                    .expect("CMake is required for the native hardcore backend")
                    .success(),
                "Native backend build failed"
            );
        };
        let args: Vec<std::ffi::OsString> = vec![
            "-S".into(),
            "native".into(),
            "-B".into(),
            native_dir.as_os_str().to_owned(),
            "-G".into(),
            "Ninja".into(),
            "-DCMAKE_BUILD_TYPE=Release".into(),
            "-DCMAKE_TRY_COMPILE_CONFIGURATION=Release".into(),
            format!("-DGUARDIAN_VERSION={}", env!("CARGO_PKG_VERSION")).into(),
            format!("-DCMAKE_CXX_COMPILER={}", compiler.path().display()).into(),
            format!("-DCMAKE_C_COMPILER={}", compiler.path().display()).into(),
        ];
        run(&args.iter().map(|a| a.as_os_str()).collect::<Vec<_>>());
        run(&["--build".as_ref(), native_dir.as_os_str()]);
        let mut tests = std::process::Command::new("ctest");
        tests
            .arg("--test-dir")
            .arg(&native_dir)
            .arg("--output-on-failure");
        for (key, value) in compiler.get_envs() {
            tests.env(key, value);
        }
        assert!(
            tests
                .status()
                .expect("CTest is required for native backend checks")
                .success(),
            "Native backend tests failed"
        );
        println!(
            "cargo:rustc-link-search=native={}",
            native_dir.join("lib").display()
        );
        for library in ["guardian_native", "safetyhook", "Zydis", "Zycore"] {
            println!("cargo:rustc-link-lib=static={library}");
        }
        cc::Build::new()
            .cpp(true)
            .std("c++17")
            .static_crt(true)
            .define(
                "GUARDIAN_VERSION",
                Some(concat!("\"", env!("CARGO_PKG_VERSION"), "\"")),
            )
            .file("src/shroudtopia.cpp")
            .compile("guardian_shroudtopia");
    }
}
