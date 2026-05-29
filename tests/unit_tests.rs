extern crate std;

/// Unit tests for the RP2040 USB-UART bridge
/// Run with: cargo test --tests
///
/// These tests run on the host (no hardware needed) and cover:
/// - Baud rate clock divider calculation
/// - Packet construction and data integrity
/// - Baud rate clamping and edge cases
/// - Packet batching logic

// ─── Baud Rate Divider ────────────────────────────────────────────────────────

/// Mirrors set_pio_baud() divider calculation from bridge/uart.rs
fn calc_div(clk_sys_freq: u32, baud: u32) -> u16 {
    let baud = baud.max(300).min(921600);
    let div = clk_sys_freq / (8 * baud);
    (div as u16).max(1)
}

#[test]
fn test_baud_115200_at_125mhz() {
    // 125_000_000 / (8 * 115200) = 135
    let div = calc_div(125_000_000, 115_200);
    assert_eq!(div, 135);
}

#[test]
fn test_baud_9600_at_125mhz() {
    // 125_000_000 / (8 * 9600) = 1627
    let div = calc_div(125_000_000, 9_600);
    assert_eq!(div, 1627);
}

#[test]
fn test_baud_230400_at_125mhz() {
    // 125_000_000 / (8 * 230400) = 67
    let div = calc_div(125_000_000, 230_400);
    assert_eq!(div, 67);
}

#[test]
fn test_baud_921600_at_125mhz() {
    // 125_000_000 / (8 * 921600) = 16
    let div = calc_div(125_000_000, 921_600);
    assert_eq!(div, 16);
}

#[test]
fn test_baud_57600_at_125mhz() {
    // 125_000_000 / (8 * 57600) = 271
    let div = calc_div(125_000_000, 57_600);
    assert_eq!(div, 271);
}

#[test]
fn test_baud_38400_at_125mhz() {
    // 125_000_000 / (8 * 38400) = 406
    let div = calc_div(125_000_000, 38_400);
    assert_eq!(div, 406);
}

#[test]
fn test_baud_19200_at_125mhz() {
    // 125_000_000 / (8 * 19200) = 813
    let div = calc_div(125_000_000, 19_200);
    assert_eq!(div, 813);
}

// ─── Baud Rate Clamping ───────────────────────────────────────────────────────

#[test]
fn test_baud_below_minimum_clamped_to_300() {
    // Anything below 300 should be clamped to 300
    let div_zero = calc_div(125_000_000, 0);
    let div_min = calc_div(125_000_000, 300);
    assert_eq!(div_zero, div_min);
}

#[test]
fn test_baud_above_maximum_clamped_to_921600() {
    // Anything above 921600 should be clamped to 921600
    let div_over = calc_div(125_000_000, 2_000_000);
    let div_max = calc_div(125_000_000, 921_600);
    assert_eq!(div_over, div_max);
}

#[test]
fn test_baud_1_clamped_to_300() {
    let div_1 = calc_div(125_000_000, 1);
    let div_300 = calc_div(125_000_000, 300);
    assert_eq!(div_1, div_300);
}

#[test]
fn test_div_never_zero() {
    // Divider of 0 would halt the PIO state machine
    for baud in [300u32, 1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 230400, 921600] {
        let div = calc_div(125_000_000, baud);
        assert!(div >= 1, "div was 0 for baud {}", baud);
    }
}

// ─── Packet Construction ──────────────────────────────────────────────────────

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
        Self { data, len: src.len() }
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
    let packets = vec![
        Packet::new(&[]),
        Packet::new(b"data"),
        Packet::new(&[]),
    ];
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

// ─── Baud Rate Change Detection ───────────────────────────────────────────────

/// Simulates the baud rate change detection logic from usb_bridge_task
fn should_signal_baud_change(current: u32, new: u32) -> bool {
    new != current && new >= 300
}

#[test]
fn test_baud_change_detected() {
    assert!(should_signal_baud_change(115200, 9600));
}

#[test]
fn test_baud_no_change_when_same() {
    assert!(!should_signal_baud_change(115200, 115200));
}

#[test]
fn test_baud_change_ignored_below_300() {
    // Host sometimes sends baud=0 or baud=1 as a reset signal — ignore it
    assert!(!should_signal_baud_change(115200, 0));
    assert!(!should_signal_baud_change(115200, 1));
    assert!(!should_signal_baud_change(115200, 299));
}

#[test]
fn test_baud_change_accepted_at_exactly_300() {
    assert!(should_signal_baud_change(115200, 300));
}

#[test]
fn test_baud_change_to_1200_accepted() {
    // Arduino bootloader reset uses 1200 baud
    assert!(should_signal_baud_change(115200, 1200));
}

#[test]
fn test_baud_change_to_921600_accepted() {
    assert!(should_signal_baud_change(115200, 921600));
}