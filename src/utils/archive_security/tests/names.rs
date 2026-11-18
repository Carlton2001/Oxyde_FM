use super::*;

#[test]
fn windows_naming_rules_apply_only_on_windows() {
    // Linux kernel sources, logs named by the hour, trailing dots.
    for name in [
        "drivers/gpu/drm/nouveau/nvkm/subdev/i2c/aux.c",
        "logs/12:30.log",
        "what?/now*.txt",
        "notes.",
    ] {
        let relative = relative_archive_path(name);
        assert_eq!(relative.is_err(), cfg!(windows), "{name:?}");
        if let Ok(relative) = relative {
            assert_eq!(relative, Path::new(name));
        }
    }
}
