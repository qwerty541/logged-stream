# logged-stream

## Table of contents

-   [Description](#description)
-   [Usage](#usage)
-   [Example](#example)
-   [License](#license)
-   [Contribution](#contribution)

## Description

`logged-stream` is a Rust library that provides a `LoggedStream` structure which can be used as a wrapper for another structures which implements `std::io::Write` and `std::io::Read` traits or their asynchronous analogues from `tokio` library to enable logging of all read and write operations, errors and drop.

`LoggedStream` structure constructs from four parts:

-   Underlying IO object, which must implement `std::io::Write` and `std::io::Read` traits or their asynchronous analogues from `tokio` library: `tokio::io::AsyncRead` and `tokio::io::AsyncWrite`.
-   Buffer formatting part, which must implement `BufferFormatter` trait provided by this library. This part of `LoggedStream` is responsible for the form you will see the input and output bytes. Currently this library provides the following implementations of `BufferFormatter` trait: `HexDecimalFormatter`, `DecimalFormatter`, `BinaryFormatter` and `OctalFormatter`. Also `BufferFormatter` is public trait so you are free to construct your own implementation.
-   Filtering part, which must implement `RecordFilter` trait provide by this library. This part of `LoggedStream` is responsible for log records filtering. Currently this library provides the following implementation of `RecordFilter` trait: `DefaultFilter` which accepts all log records and `RecordKindFilter` which accepts logs with kinds specified during construct. Also `RecordFilter` is public trait and you are free to construct your own implementation.
-   Logging part, which must implement `Logger` trait provided by this library. This part of `LoggedStream` is responsible for further work with constructed, formatter and filtered log record. For example, it can be outputted to console, written to the file, written to database, written to the memory for further use or sended by the channel. Currently this library provides the following implementations of `Logger` trait: `ConsoleLogger`, `MemoryStorageLogger` and `ChannelLogger`. Also `Logger` is public trait and you are free to construct you own implementation.

## Usage

To use `logged-stream`, add this to your `Cargo.toml`:

```toml
[dependencies]
logged-stream = "0.2"
```

or run this command in your project root:

```
$ cargo add logged-stream@0.2
```

## Example

This is a simple usage example of `LoggedStream` structure with underling `std::net::TcpStream` which connects to some echo-server as IO object, hex decimal formatter, default filter and console logger.

```rust
fn main() {
    env::set_var("RUST_LOG", "debug");
    env_logger::init();

    let mut client = LoggedStream::new(
        net::TcpStream::connect("127.0.0.1:8080").unwrap(),
        HexDecimalFormatter::new(None),
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

```
2023-04-12 20:06:04.756014 > 01:02:03:04
2023-04-12 20:06:04.756098 < 01:02:03:04
2023-04-12 20:06:04.756170 > 05:06:07:08
2023-04-12 20:06:04.756275 < 05:06:07:08
2023-04-12 20:06:04.756372 > 09:0a:0b:0c
2023-04-12 20:06:04.756514 < 09:0a:0b:0c
2023-04-12 20:06:04.756593 > 01:02:03:04
2023-04-12 20:06:04.756820 < 01:02:03:04
2023-04-12 20:06:04.756878 x Deallocated.
```

Full version of this example can be found [there](./examples/tcp-stream-console-logger.rs).

## License

Licensed under either of

-   Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
-   MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
