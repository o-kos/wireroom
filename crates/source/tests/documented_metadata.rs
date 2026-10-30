use dmd_protocol::{Rate, Stream, StreamFormat};
use std::io::Cursor;

#[test]
fn documented_wave_bytes_have_expected_source_metadata() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join("docs/replay.md");
    let document = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
    let source_section = document.split("The signal `example.wav`").nth(1).unwrap();
    let hex = source_section
        .split("```text\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let bytes: Vec<u8> = hex
        .split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).unwrap())
        .collect();
    assert_eq!(bytes.len(), 60);
    let source = dmd_source::inspect(&mut Cursor::new(bytes)).unwrap();
    assert_eq!(source.byte_size, 60);
    assert_eq!(source.frames, 8);
    assert_eq!(
        source.stream,
        Stream {
            format: StreamFormat::Real,
            channels: 1,
            rate: Rate {
                numerator: 4,
                denominator: 1
            },
        }
    );
}
