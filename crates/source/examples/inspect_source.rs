use dmd_source::Source;

fn main() -> anyhow::Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("usage: inspect_source <source.wav>"))?;
    let source = Source::open(path)?;
    let metadata = source.metadata();
    println!(
        "{} bytes; {:?}; {} frames",
        metadata.byte_size, metadata.stream, metadata.frames
    );
    Ok(())
}
