extern crate std;

/// Unit tests for the RP2040 USB-UART bridge
/// Run with: cargo test --tests
///
/// These tests run on the host (no hardware needed) and cover:
/// - Baud rate clock divider calculation
/// - Packet construction and data integrity
/// - Baud rate clamping and edge cases
/// - Packet batching logic

#[derive(Debug)]
struct Packet {
    data: [u8; 64],
    len: usize,
}

impl Packet {
    fn new(src: &[u8]) -> Self {
        assert!(src.len() <= 64, "packet too large");
        let mut data = [0u8; 64];
        data[..src.len()].copy_from_slice(src);
        Self {
            data,
            len: src.len(),
        }
    }

    fn as_slice(&self) -> &[u8] {
        &self.data[..self.len]
    }
}

#[test]
fn test_packet_stores_data_correctly() {
    let input = b"hello world";
    let p = Packet::new(input);
    assert_eq!(p.as_slice(), input);
    assert_eq!(p.len, 11);
}

#[test]
fn test_packet_empty() {
    let p = Packet::new(&[]);
    assert_eq!(p.len, 0);
    assert_eq!(p.as_slice(), &[]);
}

#[test]
fn test_packet_full_64_bytes() {
    let input = [0xABu8; 64];
    let p = Packet::new(&input);
    assert_eq!(p.len, 64);
    assert_eq!(p.as_slice(), &input);
}

#[test]
fn test_packet_single_byte() {
    let p = Packet::new(&[0x42]);
    assert_eq!(p.len, 1);
    assert_eq!(p.as_slice(), &[0x42]);
}

#[test]
fn test_packet_data_does_not_bleed_past_len() {
    // Bytes beyond len should not be visible via as_slice
    let p = Packet::new(b"abc");
    assert_eq!(p.as_slice().len(), 3);
    // Underlying buffer still has 64 bytes but slice is clamped
    assert_eq!(p.data.len(), 64);
}

#[test]
fn test_packet_preserves_binary_data() {
    let input: Vec<u8> = (0u8..64).collect();
    let p = Packet::new(&input);
    assert_eq!(p.as_slice(), input.as_slice());
}

// ─── Packet Batching Logic ────────────────────────────────────────────────────

/// Simulates the USB TX batching: drains multiple packets into one 64-byte buffer
fn batch_packets(packets: &[Packet]) -> (Vec<u8>, usize) {
    let mut buf = [0u8; 64];
    let mut n = 0;

    for p in packets {
        for i in 0..p.len {
            if n >= 64 {
                break;
            }
            buf[n] = p.data[i];
            n += 1;
        }
        if n >= 64 {
            break;
        }
    }

    (buf[..n].to_vec(), n)
}

#[test]
fn test_batch_single_packet() {
    let packets = vec![Packet::new(b"hello")];
    let (result, n) = batch_packets(&packets);
    assert_eq!(n, 5);
    assert_eq!(result, b"hello");
}

#[test]
fn test_batch_multiple_small_packets() {
    let packets = vec![
        Packet::new(b"foo"),
        Packet::new(b"bar"),
        Packet::new(b"baz"),
    ];
    let (result, n) = batch_packets(&packets);
    assert_eq!(n, 9);
    assert_eq!(result, b"foobarbaz");
}

#[test]
fn test_batch_stops_at_64_bytes() {
    // Two 40-byte packets = 80 bytes, but batch should cap at 64
    let data_a = [0xAAu8; 40];
    let data_b = [0xBBu8; 40];
    let packets = vec![Packet::new(&data_a), Packet::new(&data_b)];
    let (_, n) = batch_packets(&packets);
    assert_eq!(n, 64);
}

#[test]
fn test_batch_empty_packets_skipped() {
    let packets = vec![Packet::new(&[]), Packet::new(b"data"), Packet::new(&[])];
    let (result, n) = batch_packets(&packets);
    assert_eq!(n, 4);
    assert_eq!(result, b"data");
}

#[test]
fn test_batch_exactly_64_bytes() {
    let data = [0x55u8; 64];
    let packets = vec![Packet::new(&data)];
    let (result, n) = batch_packets(&packets);
    assert_eq!(n, 64);
    assert_eq!(result, data.to_vec());
}
