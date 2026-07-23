# logged-stream <!-- omit in toc -->

_Transparent logging wrapper for any synchronous or asynchronous Rust IO stream — inspect every byte, error, and lifecycle event that passes through._

[![Crates.io version][crates-version-badge]][crates-url]
[![Crates.io downloads][crates-downloads-badge]][crates-url]
[![Released API docs][docs-badge]][docs-url]
[![Master API docs][master-docs-badge]][master-docs-url]
[![Rust version][rust-version]][rust-url]
[![License][license-badge]][license-url]
[![Workflow Status][workflow-badge]][actions-url]
[![Lines count][sloc-badge]][scc-repo-url]
[![Cocomo][cocomo-badge]][scc-repo-url]

[crates-version-badge]: https://img.shields.io/crates/v/logged-stream.svg
[crates-downloads-badge]: https://img.shields.io/crates/d/logged-stream.svg
[crates-url]: https://crates.io/crates/logged-stream
[docs-badge]: https://docs.rs/logged-stream/badge.svg
[docs-url]: https://docs.rs/logged-stream
[license-badge]: https://img.shields.io/crates/l/logged-stream.svg
[license-url]: https://github.com/qwerty541/logged-stream/blob/master/LICENSE-MIT
[master-docs-badge]: https://img.shields.io/badge/docs-master-green.svg
[master-docs-url]: https://qwerty541.github.io/logged-stream/master/
[workflow-badge]: https://github.com/qwerty541/logged-stream/workflows/check/badge.svg
[actions-url]: https://github.com/qwerty541/logged-stream/actions
[rust-version]: https://img.shields.io/badge/rust-1.85.1%2B-lightgrey.svg?logo=rust
[rust-url]: https://blog.rust-lang.org/
[sloc-badge]: https://sloc.xyz/github/qwerty541/logged-stream/?badge-bg-color=2081C2
[cocomo-badge]: https://sloc.xyz/github/qwerty541/logged-stream/?badge-bg-color=2081C2&category=cocomo
[scc-repo-url]: https://github.com/boyter/scc

<details>
<summary>Table of contents</summary>

- [Description](#description)
- [Usage](#usage)
- [Quick start](#quick-start)
- [Example](#example)
- [Record kinds](#record-kinds)
- [Architecture](#architecture)
- [Provided implementations](#provided-implementations)
  - [Formatters (`BufferFormatter`)](#formatters-bufferformatter)
  - [Filters (`RecordFilter`)](#filters-recordfilter)
  - [Loggers (`Logger`)](#loggers-logger)
- [Custom implementations](#custom-implementations)
- [Use cases](#use-cases)
- [License](#license)
- [Contribution](#contribution)
</details>

## Description

`logged-stream` provides a single wrapper type, `LoggedStream`, that wraps any underlying IO object and logs every read, write, error, shutdown and drop that passes through it — without changing the code on either side. Because it re-implements the same IO trait it wraps, it drops straight into existing synchronous or asynchronous code.

-   **Drop-in.** Same trait in, same trait out — wrapping a stream leaves the surrounding code unchanged.
-   **Sync and async.** Works with `std::io` `Read`/`Write` streams and `tokio` `AsyncRead`/`AsyncWrite` streams alike.
-   **Replaceable parts.** Formatting, filtering and log output are each a public trait — use a built-in, or implement the trait yourself if none fits your needs.
-   **Small and safe.** A small dependency set and no `unsafe` code.

`LoggedStream` is deliberately a skeleton: it wires together four replaceable parts — the IO object you wrap, plus a formatter, a filter and a logger. Each event flows through them as `event -> Formatter -> Filter -> Logger`; see [Architecture](#architecture) for how they fit together, and [Custom implementations](#custom-implementations) to write your own.

## Usage

Add `logged-stream` to your `Cargo.toml`:

```toml
[dependencies]
logged-stream = "0.7"
```

or run:

```console
$ cargo add logged-stream@0.7
```

It requires **Rust 1.85.1 or newer** (Rust 2024 edition).

## Quick start

`LoggedStream::new` takes the stream to wrap plus the three pluggable parts — a formatter, a filter, and a logger. The snippet below wraps an in-memory [`Cursor`][cursor] (standing in for a socket or file, so it runs with no external setup), renders bytes as lowercase hexadecimal, keeps every record with `DefaultFilter`, and prints them through `ConsoleLogger`:

```rust
use log::LevelFilter;
use logged_stream::ConsoleLogger;
use logged_stream::DefaultFilter;
use logged_stream::LoggedStream;
use logged_stream::LowercaseHexadecimalFormatter;
use std::io::Cursor;
use std::io::Read;

fn main() {
    // ConsoleLogger forwards records to the `log` facade; env_logger prints them.
    env_logger::builder()
        .filter_level(LevelFilter::Debug)
        .format_timestamp_millis()
        .init();

    let mut stream = LoggedStream::new(
        Cursor::new(vec![0xde, 0xad, 0xbe, 0xef]),
        LowercaseHexadecimalFormatter::new_default(),
        DefaultFilter,
        ConsoleLogger::new_unchecked("debug"),
    );

    let mut buf = [0u8; 4];
    stream.read_exact(&mut buf).unwrap();
}
```

Running it logs the read, then the drop when `stream` goes out of scope:

```log
[2026-01-01T12:00:00.000Z DEBUG logged_stream::logger] < de:ad:be:ef
[2026-01-01T12:00:00.000Z DEBUG logged_stream::logger] x Deallocated.
```

> To see any output you also need `env_logger` and `log` in your `Cargo.toml`: `ConsoleLogger` only forwards records to the [`log`][log] facade, and any backend works.

See [Provided implementations](#provided-implementations) for the full set of formatters, filters and loggers you can swap in.

[cursor]: https://doc.rust-lang.org/std/io/struct.Cursor.html
[log]: https://crates.io/crates/log

## Example

A more realistic case: wrapping a `std::net::TcpStream` connected to an echo server. Each `write_all` logs a `>` record and each `read_exact` logs a `<` record, so both directions of the exchange are visible.

```rust
use log::LevelFilter;
use logged_stream::ConsoleLogger;
use logged_stream::DefaultFilter;
use logged_stream::LoggedStream;
use logged_stream::LowercaseHexadecimalFormatter;
use std::io::Read;
use std::io::Write;
use std::net::TcpStream;

fn main() {
    env_logger::builder()
        .filter_level(LevelFilter::Debug)
        .format_timestamp_millis()
        .init();

    // Assumes an echo server is listening on 127.0.0.1:8080 (the full example starts one).
    let mut client = LoggedStream::new(
        TcpStream::connect("127.0.0.1:8080").unwrap(),
        LowercaseHexadecimalFormatter::new_default(),
        DefaultFilter,
        ConsoleLogger::new_unchecked("debug"),
    );

    let send = [0x01, 0x02, 0x03, 0x04];
    client.write_all(&send).unwrap();
    let mut response = [0u8; 4];
    client.read_exact(&mut response).unwrap();

    let send = [0x05, 0x06, 0x07, 0x08];
    client.write_all(&send).unwrap();
    let mut response = [0u8; 4];
    client.read_exact(&mut response).unwrap();
}
```

Output to console:

```log
[2026-01-01T12:00:00.000Z DEBUG logged_stream::logger] > 01:02:03:04
[2026-01-01T12:00:00.000Z DEBUG logged_stream::logger] < 01:02:03:04
[2026-01-01T12:00:00.000Z DEBUG logged_stream::logger] > 05:06:07:08
[2026-01-01T12:00:00.000Z DEBUG logged_stream::logger] < 05:06:07:08
[2026-01-01T12:00:00.000Z DEBUG logged_stream::logger] x Deallocated.
```

The snippets above are condensed. Full runnable versions live in the [`examples/`](./examples) directory:

-   [`tcp-stream-console-logger.rs`](./examples/tcp-stream-console-logger.rs) — the synchronous example above, together with the echo server.
-   [`tokio-tcp-stream-console-logger.rs`](./examples/tokio-tcp-stream-console-logger.rs) — the same over `tokio`'s asynchronous API.
-   [`file-logger.rs`](./examples/file-logger.rs) — writing records to a file instead of the console.
-   [`composite-filters.rs`](./examples/composite-filters.rs) — combining filters with `AllFilter` / `AnyFilter`.

Full API documentation is on [docs.rs](https://docs.rs/logged-stream).

## Record kinds

Every log line begins with a single character identifying the kind of event:

| Char | Kind | Emitted when |
| --- | --- | --- |
| `<` | Read | bytes were read from the wrapped stream |
| `>` | Write | bytes were written to the wrapped stream |
| `!` | Error | a real IO error occurred (transient `WouldBlock` / `WriteZero` are skipped) |
| `-` | Shutdown | an asynchronous stream was shut down (`poll_shutdown`) |
| `x` | Drop | the wrapper was dropped (message `Deallocated.`) |
| `+` | Open | a manual marker you emit yourself with `log_open` (e.g. connection start); never produced automatically |

Every kind except `Open` is produced automatically. `Open` is a marker you record yourself with `LoggedStream::log_open` — for example `stream.log_open(format!("Established connection with {peer}"))` right after a connection is established. Like any record, it passes through the filter, so a `RecordKindFilter` that omits `Open` will suppress it.

## Architecture

`LoggedStream<S, Formatter, Filter, L>` is generic over four independent, pluggable parts. Each logged event flows through them in order:

```text
event  ->  Formatter  ->  Filter  ->  Logger
```

-   **The inner IO object (`S`).** The stream you are wrapping. Any type implementing `Read`/`Write` (or the `tokio` async equivalents) works — a socket, a file, an in-memory buffer, or your own type. `LoggedStream` implements the same IO trait `S` does, so it slots in wherever `S` was used.
-   **Formatter (`BufferFormatter`).** Turns the read and written byte buffers into the display strings you see in the log.
-   **Filter (`RecordFilter`).** Decides which records are logged. It runs on _every_ record kind, including `Shutdown` and `Drop`.
-   **Logger (`Logger`).** The sink that consumes accepted records.

All three of `BufferFormatter`, `RecordFilter` and `Logger` are public, `Send + 'static` and object-safe, with blanket implementations for `Box<...>` (and `Arc<T>` where `T: Sync` for `BufferFormatter`). You are free to supply your own implementation of any part and select one at runtime as a trait object — see [Custom implementations](#custom-implementations).

## Provided implementations

### Formatters (`BufferFormatter`)

Control how byte buffers are rendered. Each formatter stores a separator (default `:`) and exposes parallel constructors: `new`, `new_static`, `new_owned` and `new_default`.

| Formatter | Renders each byte as |
| --- | --- |
| `LowercaseHexadecimalFormatter` | lowercase hexadecimal — `0a:ff` |
| `UppercaseHexadecimalFormatter` | uppercase hexadecimal — `0A:FF` |
| `DecimalFormatter` | decimal — `10:255` |
| `OctalFormatter` | octal — `012:377` |
| `BinaryFormatter` | binary — `00001010:11111111` |

### Filters (`RecordFilter`)

Decide which records reach the logger.

| Filter | Behavior |
| --- | --- |
| `DefaultFilter` | Accepts every record. |
| `RecordKindFilter` | Accepts only the record kinds in an allow-list given at construction. |
| `AllFilter` | AND — a record passes only if every child filter accepts it (an empty list accepts everything). |
| `AnyFilter` | OR — a record passes if any child filter accepts it (an empty list rejects everything). |

### Loggers (`Logger`)

Consume each accepted record.

| Logger | Destination |
| --- | --- |
| `ConsoleLogger` | Emits records through the `log` facade. |
| `FileLogger` | Writes records to a file. |
| `MemoryStorageLogger` | Retains recent records in a bounded in-memory buffer. |
| `ChannelLogger` | Sends records over an `mpsc` channel for handling elsewhere. |

If none of the provided implementations matches your requirements, you can implement the corresponding trait yourself and pass your type to `LoggedStream` exactly like a built-in.

## Custom implementations

`LoggedStream` is deliberately a skeleton: three of its four parts are defined by public traits, and everything this crate ships is an ordinary implementation of one of them.

| Part | Trait | You must implement | Provided for you |
| --- | --- | --- | --- |
| Formatter | `BufferFormatter` | `get_separator`, `format_byte` | `format_buffer` — joins formatted bytes with your separator |
| Filter | `RecordFilter` | `check` | `fmt_debug` — override it so composite filters print more than `UnknownFilter` |
| Logger | `Logger` | `log` | — |

All three are `Send + 'static` and object-safe, so you can box them (`Box<dyn BufferFormatter>`) and choose an implementation at runtime. The fourth part needs no trait from this crate at all — anything implementing `std::io::Read`/`Write` or the `tokio` async equivalents can be wrapped, including your own types.

For example, a formatter that renders printable bytes as ASCII — handy for text protocols, which the built-in numeric formatters do not cover:

```rust
use logged_stream::BufferFormatter;

struct AsciiFormatter;

impl BufferFormatter for AsciiFormatter {
    fn get_separator(&self) -> &str {
        ""
    }

    fn format_byte(&self, byte: &u8) -> String {
        if byte.is_ascii_graphic() || *byte == b' ' {
            (*byte as char).to_string()
        } else {
            String::from(".")
        }
    }
}
```

Every record a `Logger` receives is a `Record` carrying a `kind`, a `message` and a `time`, so a custom logger can route by record kind, re-timestamp, or forward to any sink you like.

## Use cases

`LoggedStream` hands you a formatted, filterable, timestamped record of every read, write, error, shutdown, and drop that crosses a stream — without changing the code on either side of it. That makes it a building block for a range of tools and diagnostics. A few examples of what you can build:

-   **Network traffic monitoring** — wrap a `TcpStream` (synchronous or asynchronous) to record every byte a client and server exchange, for debugging protocols, tracking data flow, or auditing what actually crossed the wire.
-   **I/O debugging** — see the exact bytes moving through a file, socket, or any custom stream, alongside the errors, shutdowns, and drops around them, to pin down data corruption or where a connection unexpectedly closed.
-   **Protocol and application-traffic capture** — log the raw wire traffic of a protocol you are implementing or reverse-engineering, such as a database driver, an RPC channel, or a custom binary format. `LoggedStream` captures the bytes; decode or parse them downstream if you need higher-level detail like SQL statements.
-   **Timing and throughput diagnostics** — every record is timestamped, so you can layer your own cadence, gap, and throughput analysis on top: when reads and writes happened and how much data moved. (The timestamps are yours to correlate — the library does not measure per-call latency itself.)
-   **Transparent proxies and audit trails** — build a man-in-the-middle proxy or an audit log that relays traffic unchanged while recording it. The author's [`logged_tcp_proxy`](https://github.com/qwerty541/logged-tcp-proxy) does exactly this: it wraps each `TcpStream`, splits it into read/write halves, and prints every connection's payload. Route the records to a file, memory, or a channel through the pluggable loggers to keep a durable trail.

## License

Licensed under either of

-   Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
-   MIT license ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
