fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=app.manifest");
        println!("cargo:rerun-if-changed=app.rc");
        embed_resource::compile("app.rc", embed_resource::NONE)
            .manifest_required()
            .unwrap();
    }
}