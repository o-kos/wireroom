mod common;
use common::{chunk, fmt, standard, wave};
use dmd_protocol::{Rate, Stream, StreamFormat};
use dmd_source::{Error, Malformed, Unsupported};

#[test]
fn ordinary_pcm_profiles_have_exact_source_frames() {
    for (channels, bits, rate, frames) in [(1, 16, 4, 8), (2, 8, 48_000, 3), (1, 8, 1, 0)] {
        let samples = vec![0; frames * channels as usize * bits as usize / 8];
        let meta = inspect(wave(&[
            chunk(b"fmt ", &fmt(channels, bits, rate)),
            chunk(b"data", &samples),
        ]))
        .unwrap();
        assert_eq!(meta.frames, frames as u64);
        assert_eq!(
            meta.stream,
            Stream {
                format: StreamFormat::Real,
                channels: u32::from(channels),
                rate: Rate {
                    numerator: u64::from(rate),
                    denominator: 1
                }
            }
        );
    }
}

#[test]
fn ancillary_positions_padding_and_embedded_signatures_are_opaque() {
    let opaque = b"fmt \x00\x00\x00\x00data\xff\xff\xff\xffRIFFWAVE!!";
    let bytes = wave(&[
        chunk(b"JUNK", opaque),
        chunk(b"fmt ", &fmt(2, 8, 3)),
        chunk(b"LIST", b"odd"),
        chunk(b"data", opaque),
        chunk(b"tail", b"x"),
    ]);
    let length = bytes.len() as u64;
    let meta = inspect(bytes).unwrap();
    assert_eq!(
        (meta.byte_size, meta.frames, meta.stream.channels),
        (length, 13, 2)
    );
}

#[test]
fn malformed_fields_have_typed_reasons() {
    for (offset, bytes, reason) in [
        (22, vec![0, 0], Malformed::ZeroChannels),
        (24, vec![0; 4], Malformed::ZeroRate),
        (28, vec![0; 4], Malformed::ByteRate),
        (32, vec![0, 0], Malformed::BlockAlign),
        (24, u32::MAX.to_le_bytes().to_vec(), Malformed::Arithmetic),
    ] {
        let mut source = standard();
        source[offset..offset + bytes.len()].copy_from_slice(&bytes);
        assert!(matches!(inspect(source), Err(Error::Malformed(r)) if r == reason));
    }
    assert!(matches!(
        inspect(wave(&[
            chunk(b"fmt ", &fmt(2, 16, 1)),
            chunk(b"data", &[0; 3]),
        ])),
        Err(Error::Malformed(Malformed::PartialFrame))
    ));
}

#[test]
fn unsupported_profiles_have_typed_reasons() {
    for (offset, bytes, reason) in [
        (20, 3u16.to_le_bytes(), Unsupported::FormatTag(3)),
        (20, 0xfffeu16.to_le_bytes(), Unsupported::FormatTag(0xfffe)),
        (20, 6u16.to_le_bytes(), Unsupported::FormatTag(6)),
        (22, 3u16.to_le_bytes(), Unsupported::Channels(3)),
        (34, 24u16.to_le_bytes(), Unsupported::Bits(24)),
        (34, 32u16.to_le_bytes(), Unsupported::Bits(32)),
        (34, 0u16.to_le_bytes(), Unsupported::Bits(0)),
    ] {
        let mut source = standard();
        source[offset..offset + 2].copy_from_slice(&bytes);
        assert!(matches!(inspect(source), Err(Error::Unsupported(r)) if r == reason));
    }
    for magic in [b"RIFX", b"RF64", b"BW64"] {
        let mut source = standard();
        source[..4].copy_from_slice(magic);
        assert!(matches!(
            inspect(source),
            Err(Error::Unsupported(Unsupported::Container))
        ));
    }
    for size in [17, 20, 40] {
        let mut format = fmt(1, 16, 4);
        format.resize(size, 0);
        assert!(matches!(inspect(wave(&[chunk(b"fmt ", &format)])),
            Err(Error::Unsupported(Unsupported::FmtSize(n))) if n == size as u64));
    }
}

#[test]
fn required_chunk_order_presence_and_uniqueness() {
    let f = chunk(b"fmt ", &fmt(1, 8, 1));
    let d = chunk(b"data", &[]);
    for (chunks, reason) in [
        (vec![], Malformed::MissingFmt),
        (vec![f.clone()], Malformed::MissingData),
        (vec![d.clone(), f.clone()], Malformed::DataBeforeFmt),
        (
            vec![f.clone(), f.clone(), d.clone()],
            Malformed::DuplicateFmt,
        ),
        (vec![f.clone(), d.clone(), f], Malformed::DuplicateFmt),
        (
            vec![chunk(b"fmt ", &fmt(1, 8, 1)), d.clone(), d],
            Malformed::DuplicateData,
        ),
    ] {
        assert!(matches!(inspect(wave(&chunks)), Err(Error::Malformed(r)) if r == reason));
    }
}

#[test]
fn truncated_container_and_partial_frame_fail() {
    let bytes = standard();
    for length in [0, 11, 30, bytes.len() - 1] {
        assert!(inspect(bytes[..length].to_vec()).is_err());
    }
    let mut bad = standard();
    bad[40..44].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(inspect(bad).is_err());
}

fn inspect(bytes: Vec<u8>) -> Result<dmd_source::Metadata, Error> {
    dmd_source::inspect(&mut std::io::Cursor::new(bytes))
}

#[test]
fn maximum_integer_rate_and_short_fmt_body() {
    let meta = inspect(wave(&[
        chunk(b"fmt ", &fmt(1, 8, u32::MAX)),
        chunk(b"data", &[0]),
    ]))
    .unwrap();
    assert_eq!(
        meta.stream.rate,
        Rate {
            numerator: 4_294_967_295,
            denominator: 1
        }
    );
    assert_eq!(meta.frames, 1);
    assert!(matches!(
        inspect(wave(&[chunk(b"fmt ", &[0; 15])])),
        Err(Error::Malformed(Malformed::Truncated))
    ));
}
