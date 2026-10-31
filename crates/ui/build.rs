fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo::rerun-if-changed=assets/wireroom.rc");
    println!("cargo::rerun-if-changed=assets/wireroom.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile(
            "assets/wireroom.rc",
            embed_resource::ParamsIncludeDirs(["assets"]),
        )
        .manifest_optional()?;
    }
    Ok(())
}
