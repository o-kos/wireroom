pub fn archive(meta: &str, journal: &[u8], payload: Option<&[u8]>) -> Vec<u8> {
    archive_with_prefix(meta, journal, payload, 0)
}

pub fn archive_with_prefix(
    meta: &str,
    journal: &[u8],
    payload: Option<&[u8]>,
    blocks: usize,
) -> Vec<u8> {
    let mut deflate = [0, 0, 0, 255, 255].repeat(blocks);
    let method = if blocks == 0 { 0u16 } else { 8 };
    let compressed = if blocks == 0 {
        journal
    } else {
        let len = u16::try_from(journal.len()).unwrap();
        deflate.push(1);
        deflate.extend(len.to_le_bytes());
        deflate.extend((!len).to_le_bytes());
        deflate.extend(journal);
        &deflate
    };
    let mut members = vec![
        ("metadata.json", meta.as_bytes(), meta.as_bytes(), 0u16),
        ("journal.ndjson", journal, compressed, method),
    ];
    if let Some(bytes) = payload {
        members.push(("payloads/files/message.bin", bytes, bytes, 0));
        members.push(("payloads/files/second.bin", bytes, bytes, 0));
    }
    let count = members.len() as u16;
    let mut zip = Vec::new();
    let mut central = Vec::new();
    for (name, data, compressed, method) in members {
        let offset = zip.len() as u32;
        let crc = crc32fast::hash(data);
        let n = data.len() as u32;
        zip.extend(0x04034b50u32.to_le_bytes());
        zip.extend(20u16.to_le_bytes());
        zip.extend(0x800u16.to_le_bytes());
        zip.extend(method.to_le_bytes());
        zip.extend([0; 4]);
        zip.extend(crc.to_le_bytes());
        zip.extend((compressed.len() as u32).to_le_bytes());
        zip.extend(n.to_le_bytes());
        zip.extend((name.len() as u16).to_le_bytes());
        zip.extend([0; 2]);
        zip.extend(name.bytes());
        zip.extend(compressed);
        central.extend(0x02014b50u32.to_le_bytes());
        central.extend(20u16.to_le_bytes());
        central.extend(20u16.to_le_bytes());
        central.extend(0x800u16.to_le_bytes());
        central.extend(method.to_le_bytes());
        central.extend([0; 4]);
        central.extend(crc.to_le_bytes());
        central.extend((compressed.len() as u32).to_le_bytes());
        central.extend(n.to_le_bytes());
        central.extend((name.len() as u16).to_le_bytes());
        central.extend([0; 12]);
        central.extend(offset.to_le_bytes());
        central.extend(name.bytes());
    }
    let offset = zip.len() as u32;
    let size = central.len() as u32;
    zip.extend(central);
    zip.extend(0x06054b50u32.to_le_bytes());
    zip.extend([0; 4]);
    zip.extend(count.to_le_bytes());
    zip.extend(count.to_le_bytes());
    zip.extend(size.to_le_bytes());
    zip.extend(offset.to_le_bytes());
    zip.extend([0; 2]);
    zip
}
