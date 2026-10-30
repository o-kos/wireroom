pub fn archive(meta: &str, journal: &[u8], payload: Option<&[u8]>) -> Vec<u8> {
    let mut members = vec![
        ("metadata.json", meta.as_bytes()),
        ("journal.ndjson", journal),
    ];
    if let Some(bytes) = payload {
        members.push(("payloads/files/message.bin", bytes));
    }
    let mut zip = Vec::new();
    let mut central = Vec::new();
    for (name, data) in members {
        let offset = zip.len() as u32;
        let crc = crc32fast::hash(data);
        let n = data.len() as u32;
        zip.extend(0x04034b50u32.to_le_bytes());
        zip.extend(20u16.to_le_bytes());
        zip.extend(0x800u16.to_le_bytes());
        zip.extend([0; 6]);
        zip.extend(crc.to_le_bytes());
        zip.extend(n.to_le_bytes());
        zip.extend(n.to_le_bytes());
        zip.extend((name.len() as u16).to_le_bytes());
        zip.extend([0; 2]);
        zip.extend(name.bytes());
        zip.extend(data);
        central.extend(0x02014b50u32.to_le_bytes());
        central.extend(20u16.to_le_bytes());
        central.extend(20u16.to_le_bytes());
        central.extend(0x800u16.to_le_bytes());
        central.extend([0; 6]);
        central.extend(crc.to_le_bytes());
        central.extend(n.to_le_bytes());
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
    let count = if payload.is_some() { 3u16 } else { 2 };
    zip.extend(count.to_le_bytes());
    zip.extend(count.to_le_bytes());
    zip.extend(size.to_le_bytes());
    zip.extend(offset.to_le_bytes());
    zip.extend([0; 2]);
    zip
}
