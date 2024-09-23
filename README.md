# logged-stream <!-- omit in toc -->

[![Crates.io][crates-badge]][crates-url]
[![Released API docs][docs-badge]][docs-url]
[![Master API docs][master-docs-badge]][master-docs-url]
![Rust version][rust-version]
![License][license-badge]
[![Workflow Status][workflow-badge]][actions-url]

[crates-badge]: https://img.shields.io/crates/v/logged-stream.svg
[crates-url]: https://crates.io/crates/logged-stream
[docs-badge]: https://docs.rs/logged-stream/badge.svg
[docs-url]: https://docs.rs/logged-stream
[license-badge]: https://img.shields.io/crates/l/logged-stream.svg
[master-docs-badge]: https://img.shields.io/badge/docs-master-green.svg
[master-docs-url]: https://qwerty541.github.io/logged-stream/master/
[workflow-badge]: https://github.com/qwerty541/logged-stream/workflows/check/badge.svg
[actions-url]: https://github.com/qwerty541/logged-stream/actions
[rust-version]: https://img.shields.io/badge/rust-1.71.1%2B-lightgrey.svg?logo=rust

<details>
<summary>Table of contents</summary>

- [Description](#description)
  - [Structure](#structure)
  - [Use Cases](#use-cases)
- [Usage](#usage)
- [Example](#example)
- [License](#license)
- [Contribution](#contribution)
</details>

## Description

### Structure

`logged-stream` is a Rust library that provides a `LoggedStream` structure which can be used as a wrapper for underlying IO object which implements `std::io::Write` and `std::io::Read` traits or their asynchronous analogues from `tokio` library to enable logging of all read and write operations, errors and drop.

`LoggedStream` structure constructs from four parts:

-   Underlying IO object, which must implement `std::io::Write` and `std::io::Read` traits or their asynchronous analogues from `tokio` library: `tokio::io::AsyncRead` and `tokio::io::AsyncWrite`.
-   Buffer formatting part, which must implement `BufferFormatter` trait provided by this library. This part of `LoggedStream` is responsible for the form you will see the input and output bytes. Currently this library provides the following implementations of `BufferFormatter` trait: `LowercaseHexadecimalFormatter`, `UppercaseHexadecimalFormatter`, `DecimalFormatter`, `BinaryFormatter` and `OctalFormatter`. Also `BufferFormatter` is public trait so you are free to construct your own implementation.
-   Filtering part, which must implement `RecordFilter` trait provide by this library. This part of `LoggedStream` is responsible for log records filtering. Currently this library provides the following implementation of `RecordFilter` trait: `DefaultFilter` which accepts all log records and `RecordKindFilter` which accepts logs with kinds specified during construct. Also `RecordFilter` is public trait and you are free to construct your own implementation.
-   Logging part, which must implement `Logger` trait provided by this library. This part of `LoggedStream` is responsible for further work with constructed, formatter and filtered log record. For example, it can be outputted to console, written to the file, written to database, written to the memory for further use or sended by the channel. Currently this library provides the following implementations of `Logger` trait: `ConsoleLogger`, `MemoryStorageLogger`, `ChannelLogger` and `FileLogger`. Also `Logger` is public trait and you are free to construct your own implementation.

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
logged-stream = "0.4"
```

or run the following Cargo command in your project directory:

```
$ cargo add logged-stream@0.4
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
