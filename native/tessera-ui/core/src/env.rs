//! Tessera's environment names (`TESSERA_DEBUG`, `TESSERA_ROOT`, …): the name
//! from before the rename (`IRICE_…`) still works, the new one wins when both
//! are set (docs/rename-to-tessera.md).

/// `TESSERA_<name>`, else `IRICE_<name>`
pub fn var(name: &str) -> Option<String> {
    std::env::var(format!("TESSERA_{name}")).ok().or_else(|| std::env::var(format!("IRICE_{name}")).ok())
}

/// Set, under either name
pub fn is_set(name: &str) -> bool {
    var(name).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_name_wins_and_old_name_still_works() {
        // (names of their own: tests run on threads, the environment is shared)
        std::env::set_var("IRICE_ENVTEST_A", "old");
        assert_eq!(var("ENVTEST_A").as_deref(), Some("old"));
        std::env::set_var("TESSERA_ENVTEST_A", "new");
        assert_eq!(var("ENVTEST_A").as_deref(), Some("new"));
        assert!(is_set("ENVTEST_A"));
        assert!(!is_set("ENVTEST_NEVER_SET"));
    }
}
