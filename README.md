# RP2040 USB-UART Bridge

## Objective 

The Objective of this project is to replace cheap or expensive USB to UART converters like CP2102 or FTDI232
with just on USB connection, avoiding excessive use o wires and other peripherals, given the small amount of 
USB ports on newer laptops.  

## Overview

A 3-port USB CDC to UART bridge running on the **RP2040 zero** board or cheap default **RP2040 development boards**, built with [Embassy](https://embassy.dev/) async embedded Rust.

Exposes **3 independent serial ports** over a single USB connection, each bridged to a dedicated UART. Features a breathing WS2812 RGB LED running on core 1, isolated from the UART workload on core 0.

---

## Features

- 3x USB CDC ACM serial ports (appear as `/dev/ttyACM0`, `/dev/ttyACM1`, `/dev/ttyACM2` on Linux)
- 3x PIO-based software UARTs with 115200 baud
- Packet-batched bridging for efficient USB throughput
- USB disconnect/reconnect handling with channel flush
- WS2812 RGB LED breathing animation on core 1 (isolated from UART interrupts) 
- Bit-banged WS2812 driver with timing corruption detection and retry
- Full support for RP2040 zero board and default rp2040 dev board
---

## Hardware

| Component | Details |
|-----------|---------|
| MCU | RP2040 (dual-core Cortex-M0+) |
| Board | RP2040 zero |
| LED | WS2812B on GPIO16 |
| USB | Full-speed USB 2.0 (native RP2040 USB) |

Or also the common dev board. 

| Component | Details |
|-----------|---------|
| MCU | RP2040 (dual-core Cortex-M0+) |
| Board | RP2040 zero |
| LED | Blink (Keep Alive) on GPIO25 |
| USB | Full-speed USB 2.0 (native RP2040 USB) |


### UART Pin Mapping

| Port | RX Pin | TX Pin | PIO Instance |
|------|--------|--------|--------------|
| UART 1 | GPIO27 | GPIO2 | PIO0 SM0/SM1 |
| UART 2 | GPIO28 | GPIO1 | PIO0 SM2/SM3 |
| UART 3 | GPIO12 | GPIO11 | PIO1 SM0/SM1 |

> All RX pins have pull-ups enabled to prevent noise when nothing is connected.

---

## Architecture

```
USB Host
   │
   ├── CDC ACM Class 0 ──► usb_bridge_task ──► BridgeChannels 0 ──► uart_bridge_task (PIO0 SM0/SM1)
   ├── CDC ACM Class 1 ──► usb_bridge_task ──► BridgeChannels 1 ──► uart_bridge_task (PIO0 SM2/SM3)
   └── CDC ACM Class 2 ──► usb_bridge_task ──► BridgeChannels 2 ──► uart_bridge_task (PIO1 SM0/SM1)

Core 0: USB task + 3x usb_bridge_task + 3x uart_bridge_task
Core 1: ws2812_task (LED, fully isolated)
```

### Bridge Channels

Each `BridgeChannels` instance holds two async channels and a baud rate signal:
- `usb_to_uart`: packets from USB host → UART TX
- `uart_to_usb`: packets from UART RX → USB host

Each channel carries `Packet { data: [u8; 64], len: usize }` with a depth of 4, giving ~576 bytes per bridge.

### UART Batching

The UART RX side reads the first byte, then waits up to **2ms** for additional bytes before packaging them into a single packet. This batches bursts of bytes into fewer, larger USB packets — improving throughput significantly over byte-by-byte forwarding.

### USB Disconnect Handling

When USB disconnects:
1. `usb_rx` detects the error and fires a `Signal`
2. `usb_tx` is woken via `select()` on the signal and exits cleanly
3. Both channels are flushed to discard stale data
4. The outer loop calls `wait_connection()` until USB reconnects

### WS2812 Driver

`SingleWs2812` is a blocking bit-banged driver using `cortex_m::asm::delay` for precise timing. It includes a corruption detector — if the total write time exceeds 38µs (indicating an interrupt preempted the bit-bang sequence), it retries after a 60µs reset pulse. Running on core 1 eliminates preemption from USB/UART interrupts.

---

## Project Structure

```
src/
├── main.rs              # Entry point, peripheral init, task spawning
├── bsp.rs               # Board support (reserved)
├── bridge/
│   ├── mod.rs
│   ├── channels.rs      # BridgeChannels and Packet types
│   └── uart.rs          # Generic async UART bridge logic
├── drivers/
│   ├── mod.rs
│   └── led.rs           # Bit-banged WS2812 driver with timing retry
│   └── uart.rs          # The UART creation abstraction for the PIO UART RX and TX
│   └── usb.rs           # The USB creation abstraction
└── tasks/
    ├── mod.rs
    ├── led.rs           # WS2812 breathing animation task (core 1)
    ├── uart.rs          # PIO UART bridge tasks
    └── usb.rs           # USB device task + CDC bridge tasks
```

---

## Building

### Prerequisites

- Rust with `thumbv6m-none-eabi` target:
  ```bash
  rustup target add thumbv6m-none-eabi
  ```
- `flip-link` for stack overflow protection (optional but recommended):
  ```bash
  cargo install flip-link
  ```
- `probe-rs` for flashing:
  ```bash
  cargo install probe-rs-tools
  ```

### Build

```bash
cargo build --release
```

### Flash

With a debug probe (SWD):
In case you are using and rp2040-zero board
```bash
cargo run --release
```
In case you are using an default rp2040 board
```bash
cargo run --release --no-default-features --features rp-2040-board-dev
```
Via UF2 (hold BOOTSEL while plugging in USB):
```bash
cargo build --release
elf2uf2-rs target/thumbv6m-none-eabi/release/rp2040-project firmware.uf2
# Copy firmware.uf2 to the RPI-RP2 mass storage device
```

---

## Memory Layout

The project uses a custom `memory.x` with an increased stack size to accommodate the deep async call stacks of USB + 3 CDC classes + 3 UART bridges running concurrently:

```
_stack_size = 0x8000;  // 32KB stack
```

The RP2040 has 264KB SRAM total. Static allocations (task pools, USB descriptors, bridge channels) use approximately 20KB, leaving ample room for the stack.

---

## Known Limitations

- No hardware flow control (RTS/CTS)
- Parity and stop bit settings from CDC line coding are acknowledged but not enforced by the PIO UART program
- The WS2812 driver is blocking — the LED task must run on core 1 to avoid interfering with UART timing

---

## Dependencies

| Crate | Purpose |
|-------|---------|
| `embassy-rp` | RP2040 HAL |
| `embassy-executor` | Async executor (dual-core) |
| `embassy-usb` | USB device stack |
| `embassy-sync` | Channels and signals |
| `embassy-time` | Async timers |
| `embassy-futures` | `join`, `select` combinators |
| `embedded-io-async` | Async IO traits |
| `static_cell` | Safe static initialization |
| `rp-pac` | Direct PIO register access for runtime baud rate changes |
| `defmt` + `defmt-rtt` | Logging over RTT |
| `libm` | `sin`, `pow` for LED animation |

All Embassy crates are pinned to the same git commit to ensure ABI compatibility.