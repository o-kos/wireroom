//! `cargo run -p dmd-replay --example inspect -- recording.replay source_frames`
use dmd_protocol::{Descriptor, Event};
use dmd_replay::{Record, Recording};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let archive = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("provide an archive pathname"))?;
    let frames = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("provide independently derived source frames"))?;
    let frames = frames
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("source frames must be UTF-8"))?
        .parse()?;
    let recording = Recording::open(archive, frames)?;
    eprintln!("Recorded module: {}", recording.metadata().module);
    let mut journal = recording.journal()?;
    while let Some(record) = journal.next_record()? {
        match record {
            Record::Event(Event::Result(result)) => {
                inspect_assets(&recording, &result)?;
            }
            Record::Boundary { from, to } => eprintln!("Validated block: {from}..{to}"),
            Record::Finalization => eprintln!("Finalization transition"),
            Record::Event(_) => {}
        }
    }
    Ok(())
}

fn inspect_assets(
    recording: &Recording,
    result: &dmd_protocol::ResultEvent,
) -> Result<(), dmd_replay::Error> {
    for descriptor in result.binary.iter().flatten() {
        if !matches!(descriptor, Descriptor::File { .. }) {
            continue;
        }
        let mut asset = recording.asset(descriptor)?;
        let mut bytes = [0; 8192];
        while asset.read_chunk(&mut bytes)? != 0 {}
    }
    Ok(())
}
