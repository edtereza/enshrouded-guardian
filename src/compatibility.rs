//! A pattern match identifies code; it does not validate the engine's data layout.
//! Extra fixed-layout features require a verified build. Core hooks may run on
//! divergent builds only after all required patterns and call-site checks pass.
use std::sync::atomic::{AtomicBool, Ordering};

pub const VERIFIED_BUILD: &str = "6aeb143e";
static VERIFIED_LAYOUT: AtomicBool = AtomicBool::new(false);
static VERIFIED_PATTERNS: AtomicBool = AtomicBool::new(false);

pub fn supported(build: &str) -> bool { build == VERIFIED_BUILD }

pub fn verify(build: &str) -> bool {
    let allowed = supported(build);
    VERIFIED_LAYOUT.store(allowed, Ordering::Release);
    allowed
}

pub fn ready() -> bool { VERIFIED_LAYOUT.load(Ordering::Acquire) }
pub fn patterns_ready() -> bool { VERIFIED_PATTERNS.load(Ordering::Acquire) }
pub fn patterns_verified() { VERIFIED_PATTERNS.store(true, Ordering::Release); }
pub fn disable() {
    VERIFIED_LAYOUT.store(false, Ordering::Release);
    VERIFIED_PATTERNS.store(false, Ordering::Release);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_builds_do_not_authorize_fixed_layout_access() {
        assert!(supported("6aeb143e"));
        for build in ["", "00000000", "6aeb143f", "6aeb143e-modified", "6AEB143E"] {
            assert!(!supported(build), "unverified build: {build}");
        }
    }
    #[test]
    fn core_pattern_and_extra_layout_guards_are_independent() {
        disable();
        assert!(!verify("unverified-build"));
        assert!(!patterns_ready());
        patterns_verified();
        assert!(patterns_ready());
        assert!(!ready());
        disable();
        // Disabled callbacks must not dereference even invalid input pointers.
        let pointer = std::ptr::NonNull::<u8>::dangling().as_ptr();
        unsafe {
            assert!(!crate::state::entrada(pointer));
            assert!(crate::state::saida_antes(pointer).is_none());
            crate::state::saida_depois(pointer, 64);
            crate::state::sessao(pointer);
        }
        assert!(verify(VERIFIED_BUILD));
        assert!(ready());
        assert!(!patterns_ready());
        disable();
    }
}
