//! Target-observed NTDLL exports used by low-level loader code.

use vnrt_win32::{ApiKey, ApiRegistry, UnsupportedApi};

const MODULE: &str = "ntdll.dll";

/// Register the currently observed NTDLL compatibility surface.
pub fn register(registry: &mut ApiRegistry) {
    for (name, feature) in [
        ("NtContinue", "NtContinue context restoration"),
        ("NtCreateSection", "NT section creation"),
        ("NtMapViewOfSection", "NT section view mapping"),
        ("NtQuerySection", "NtQuerySection metadata"),
        ("RtlNtStatusToDosError", "NTSTATUS to Win32 error mapping"),
        (
            "RtlDosPathNameToNtPathName_U",
            "DOS path to NT path conversion",
        ),
        ("RtlFreeUnicodeString", "NT Unicode string release"),
    ] {
        registry.register(ApiKey::new(MODULE, name), UnsupportedApi::new(feature));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registers_observed_loader_exports() {
        let mut registry = ApiRegistry::new();
        register(&mut registry);
        for name in [
            "NtContinue",
            "NtCreateSection",
            "NtMapViewOfSection",
            "NtQuerySection",
            "RtlNtStatusToDosError",
            "RtlDosPathNameToNtPathName_U",
            "RtlFreeUnicodeString",
        ] {
            assert!(registry.resolve(&ApiKey::new(MODULE, name)).is_some());
        }
    }
}
