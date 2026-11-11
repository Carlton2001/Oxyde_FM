fn main() {
    println!("cargo:rerun-if-changed=assets/windows/oxyde.rc");
    println!("cargo:rerun-if-changed=assets/icons/oxyde.ico");
    println!("cargo:rerun-if-changed=assets/windows/worker.rc");
    println!("cargo:rerun-if-changed=assets/icons/worker.ico");
    // GPUI loads icon resource 1 for the window class, hence the taskbar too.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let version = env!("CARGO_PKG_VERSION");
        let numbers = version.split(['.', '-', '+']).take(3).map(|part| part.parse::<u16>().unwrap_or(0).to_string()).collect::<Vec<_>>().join(",");
        let macros = [format!("OXYDE_VERSION_NUMBERS={numbers},0"), format!("OXYDE_VERSION_TEXT=\"{version}\"")];
        embed_resource::compile_for("assets/windows/oxyde.rc", ["oxyde"], macros.iter().map(String::as_str))
            .manifest_required()
            .unwrap();
        // The launcher of folders is a helper executable like the workers.
        let workers = ["oxyde-image-worker", "oxyde-archive-io-worker", "oxyde-archive-list-worker", "oxyde-folder-launcher"];
        embed_resource::compile_for("assets/windows/worker.rc", workers, embed_resource::NONE)
            .manifest_required()
            .unwrap();
    }
}
