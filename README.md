# logged-stream <!-- omit in toc -->

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
  - [Architecture](#architecture)
  - [Provided implementations](#provided-implementations)
  - [Use Cases](#use-cases)
- [Usage](#usage)
- [Example](#example)
- [License](#license)
- [Contribution](#contribution)
</details>

## Description

`logged-stream` provides a single wrapper type, `LoggedStream`, that wraps any underlying IO object and logs every read, write, error, shutdown and drop that passes through it. The wrapper re-implements the same IO trait it wraps — `std::io::Read` / `std::io::Write`, or the `tokio` asynchronous analogues `tokio::io::AsyncRead` / `tokio::io::AsyncWrite` — so it is a drop-in replacement for the stream it decorates and works transparently in both synchronous and asynchronous code.

### Architecture

`LoggedStream<S, Formatter, Filter, L>` is generic over four independent, pluggable parts. Each logged event flows through them in order:

```text
event  ->  Formatter  ->  Filter  ->  Logger
```

-   **The inner IO object (`S`).** The stream you are wrapping. `LoggedStream` implements the same IO trait `S` does, so it slots in wherever `S` was used.
-   **Formatter (`BufferFormatter`).** Turns the read and written byte buffers into the display strings you see in the log.
-   **Filter (`RecordFilter`).** Decides which records are logged. It runs on _every_ record kind, including `Shutdown` and `Drop`.
-   **Logger (`Logger`).** The sink that consumes accepted records.

All three of `BufferFormatter`, `RecordFilter` and `Logger` are public, `Send + 'static` and object-safe, with blanket implementations for `Box<...>` (and `Arc<...>` for `BufferFormatter`). You are free to supply your own implementation of any part and select one at runtime as a trait object.

### Provided implementations

#### Formatters (`BufferFormatter`)

Control how byte buffers are rendered. Each formatter stores a separator (default `:`) and exposes parallel constructors: `new`, `new_static`, `new_owned` and `new_default`.

| Formatter | Renders each byte as |
| --- | --- |
| `LowercaseHexadecimalFormatter` | lowercase hexadecimal — `0a:ff` |
| `UppercaseHexadecimalFormatter` | uppercase hexadecimal — `0A:FF` |
| `DecimalFormatter` | decimal — `10:255` |
| `OctalFormatter` | octal — `012:377` |
| `BinaryFormatter` | binary — `00001010:11111111` |

#### Filters (`RecordFilter`)

Decide which records reach the logger.

| Filter | Behavior |
| --- | --- |
| `DefaultFilter` | Accepts every record. |
| `RecordKindFilter` | Accepts only the record kinds in an allow-list given at construction. |
| `AllFilter` | AND — a record passes only if every child filter accepts it (an empty list accepts everything). |
| `AnyFilter` | OR — a record passes if any child filter accepts it (an empty list rejects everything). |

#### Loggers (`Logger`)

Consume each accepted record.

| Logger | Destination |
| --- | --- |
| `ConsoleLogger` | Emits records through the `log` facade. |
| `FileLogger` | Writes records to a file. |
| `MemoryStorageLogger` | Retains recent records in a bounded in-memory buffer. |
| `ChannelLogger` | Sends records over an `mpsc` channel for handling elsewhere. |

### Use Cases

- Network Traffic Monitoring:
   - Monitor and log all incoming and outgoing network traffic in a server or client application.
   - Useful for debugging network protocols, tracking data exchange, and ensuring security compliance.
- Debugging I/O Operations:
   - Log all read and write operations to diagnose issues with file or network I/O.
   - Helps in identifying bottlenecks, data corruption, and unexpected behavior in I/O operations.
- Performance Analysis:
   - Analyze the performance of I/O operations by logging the time taken for each read/write operation.
   - Helps in identifying performance issues and optimizing I/O-intensive applications.
- Database Activity Logging:
  - Log all interactions with a database, including queries, updates, and transaction details.
  - Helps in database performance tuning, debugging query issues, and maintaining audit logs.
- Proxy Servers:
  - Implement logging in proxy servers to monitor and log all forwarded traffic.
  - Useful for debugging proxy behavior and ensuring proper data routing.

## Usage

To use `logged-stream`, add the following line to your `Cargo.toml`:

```toml
[dependencies]
logged-stream = "0.6"
```

or run the following Cargo command in your project directory:

```
$ cargo add logged-stream@0.6
```

## Example

This is a simple usage example of `LoggedStream` structure with `std::net::TcpStream` as underling IO object which connects to some echo-server, lowercase hexadecimal formatter, default filter and console logger.

```rust
fn main() {
    env::set_var("RUST_LOG", "debug");
    env_logger::init();

    let mut client = LoggedStream::new(
        net::TcpStream::connect("127.0.0.1:8080").unwrap(),
        LowercaseHexadecimalFormatter::new(None),
        DefaultFilter::default(),
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

    let send = [0x09, 0x0a, 0x0b, 0x0c];
    client.write_all(&send).unwrap();
    let mut response = [0u8; 4];
    client.read_exact(&mut response).unwrap();

    let send = [0x01, 0x02, 0x03, 0x04];
    client.write_all(&send).unwrap();
    let mut response = [0u8; 4];
    client.read_exact(&mut response).unwrap();
}
```

Output to console:

```log
[2023-04-18T08:18:45.895Z DEBUG logged_stream::logger] > 01:02:03:04
[2023-04-18T08:18:45.895Z DEBUG logged_stream::logger] < 01:02:03:04
[2023-04-18T08:18:45.895Z DEBUG logged_stream::logger] > 05:06:07:08
[2023-04-18T08:18:45.895Z DEBUG logged_stream::logger] < 05:06:07:08
[2023-04-18T08:18:45.895Z DEBUG logged_stream::logger] > 09:0a:0b:0c
[2023-04-18T08:18:45.896Z DEBUG logged_stream::logger] < 09:0a:0b:0c
[2023-04-18T08:18:45.896Z DEBUG logged_stream::logger] > 01:02:03:04
[2023-04-18T08:18:45.896Z DEBUG logged_stream::logger] < 01:02:03:04
[2023-04-18T08:18:45.896Z DEBUG logged_stream::logger] x Deallocated.
```

Full version of this example can be found [there](./examples/tcp-stream-console-logger.rs).

Same example, but rewritten using asynchronous API, can be found [there](./examples/tokio-tcp-stream-console-logger.rs).

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
