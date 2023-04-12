# logged-stream

## Table of contents

-   [Description](#description)
-   [Usage](#usage)
-   [Example](#example)
-   [License](#license)

## Description

`logged-stream` is a Rust library that provides a wrapper `LoggedStream` for structures which implements `std::io::Write` and `std::io::Read` traits or their asynchronous analogues from `tokio` to enable logging of all read and write operations, errors and drop.

`LoggedStream` structure constructs from three parts:

-   Underlying IO object, which implements `std::io::Write` and `std::io::Read` traits or their asynchronous analogues from `tokio`: `tokio::io::AsyncRead` and `tokio::io::AsyncWrite`.
-   Buffer formatter, which implements `BufferFormatter` trait provided by this library. This part of `LoggedStream` is responsible for the form you will see the input and output bytes. Currently this library provides the following implementations of `BufferFormatter` trait: `HexDecimalFormatter`, `DecimalFormatter`, `BinaryFormatter` and `OctalFormatter`. Also `BufferFormatter` is public trait so you are free to construct your own implementation.
-   Logger, which implements `Logger` trait provided by this library. This part of `LoggedStream` is responsible for further work with constructed and formatter log record. For example, it can be outputted to console, written to the file, written to database, written to the memory for further use or sended by the channel. Currently this library provides the following implementations of `Logger` trait: `ConsoleLogger`, `MemoryStorageLogger` and `ChannelLogger`. Also `Logger` is public trait and you are free to construct you own implementation.

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
logged-stream = "0.1"
```

or run command in your project root:

```
$ cargo add logged-stream@0.1
```

## Example

This is a simple usage example of `LoggedStream` with underling `std::net::TcpStream` which connects to some echo-server, hex decimal formatter and console logger.

```rust
fn main() {
    env::set_var("RUST_LOG", "debug");
    env_logger::init();

    let mut client = LoggedStream::new(
        net::TcpStream::connect("127.0.0.1:8080").unwrap(),
        HexDecimalFormatter::new(None),
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

Output:

```
2023-04-12 20:06:04.756014 > 01:02:03:04
2023-04-12 20:06:04.756098 < 01:02:03:04
2023-04-12 20:06:04.756170 > 05:06:07:08
2023-04-12 20:06:04.756275 < 05:06:07:08
2023-04-12 20:06:04.756372 > 09:0a:0b:0c
2023-04-12 20:06:04.756514 < 09:0a:0b:0c
2023-04-12 20:06:04.756593 > 01:02:03:04
2023-04-12 20:06:04.756820 < 01:02:03:04
2023-04-12 20:06:04.756878 x Connection socket deallocated.
```

Full version of this example can be found [there](./examples/tcp-stream-console-logger.rs).

## License

Licensed under either of [Apache License, Version 2.0](./LICENSE-APACHE) or [MIT](./LICENSE-MIT) license at your option.
