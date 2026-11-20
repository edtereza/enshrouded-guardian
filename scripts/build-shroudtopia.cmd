@echo off
where cargo >nul 2>&1 || (echo Rust/Cargo must be on PATH. & exit /b 1)
where cl >nul 2>&1 || (echo Run from an x64 Visual Studio Developer Command Prompt. & exit /b 1)
cargo test --locked --target x86_64-pc-windows-msvc --features shroudtopia || exit /b 1
cargo build --locked --release --target x86_64-pc-windows-msvc --features shroudtopia
