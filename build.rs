fn main() {
    println!("cargo:rerun-if-changed=assets/windows/oxyde.rc");
    println!("cargo:rerun-if-changed=assets/icons/oxyde.ico");
    println!("cargo:rerun-if-changed=assets/windows/worker.rc");
    println!("cargo:rerun-if-changed=assets/icons/worker.ico");
    // GPUI loads icon resource 1 for the window class, hence the taskbar too.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile_for("assets/windows/oxyde.rc", ["oxyde"], embed_resource::NONE)
            .manifest_required()
            .unwrap();
        // The launcher of folders is a helper executable like the workers.
        let workers = ["oxyde-image-worker", "oxyde-archive-io-worker", "oxyde-archive-list-worker", "oxyde-folder-launcher"];
        embed_resource::compile_for("assets/windows/worker.rc", workers, embed_resource::NONE)
            .manifest_required()
            .unwrap();
    }
}
