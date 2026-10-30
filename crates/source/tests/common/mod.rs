pub fn fmt(channels: u16, bits: u16, rate: u32) -> Vec<u8> {
    let align = channels * (bits / 8);
    let mut bytes = Vec::new();
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(channels.to_le_bytes());
    bytes.extend(rate.to_le_bytes());
    bytes.extend(rate.checked_mul(u32::from(align)).unwrap().to_le_bytes());
    bytes.extend(align.to_le_bytes());
    bytes.extend(bits.to_le_bytes());
    bytes
}

pub fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut bytes = id.to_vec();
    bytes.extend((body.len() as u32).to_le_bytes());
    bytes.extend(body);
    if !body.len().is_multiple_of(2) {
        bytes.push(0xa5);
    }
    bytes
}

pub fn wave(chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = b"RIFF\0\0\0\0WAVE".to_vec();
    for chunk in chunks {
        bytes.extend(chunk);
    }
    let size = (bytes.len() - 8) as u32;
    bytes[4..8].copy_from_slice(&size.to_le_bytes());
    bytes
}

pub fn standard() -> Vec<u8> {
    wave(&[chunk(b"fmt ", &fmt(1, 16, 4)), chunk(b"data", &[0; 16])])
}
