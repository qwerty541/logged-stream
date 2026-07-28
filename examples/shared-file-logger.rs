//! Several `LoggedStream`s writing to one shared log file.
//!
//! This is the pattern behind a proxy or server that handles many connections at once and wants a
//! single readable log of all of them: every connection gets its own `LoggedStream` with its own
//! `FileLogger`, all of them appending to the same file, each tagged with a prefix so the lines can
//! be told apart afterwards.
//!
//! Two details make this work, and both are easy to get wrong:
//!
//! -   **The file must be opened in append mode.** `FileLogger::open` does that for you. Handing
//!     each logger an independently opened non-append file (for example from `File::create`) gives
//!     every logger its own write offset starting at zero, and they silently overwrite each other —
//!     leaving a plausible-looking file that is missing most of its records.
//! -   **Each record is written with a single `write_all` call.** `FileLogger` renders the whole
//!     line up front for exactly this reason, so two loggers writing at the same moment cannot
//!     splice half of one line into the middle of another.
//!
//! The prefix is written after the timestamp and immediately before the record kind character, so
//! the timestamp still leads every line and the file stays sortable.
//!
//! Run with `cargo run --example shared-file-logger`; the resulting log is printed at the end.

use logged_stream::DefaultFilter;
use logged_stream::FileLogger;
use logged_stream::LoggedStream;
use logged_stream::LowercaseHexadecimalFormatter;
use std::fs;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net;

const SERVER_ADDRESS: &str = "127.0.0.1:8080";
const LOG_PATH: &str = "./examples/shared-traffic.log";
const CONNECTIONS: u8 = 3;

/// Echo server side of the connection: read whatever arrives and send it straight back.
async fn handle_connection(mut stream: net::TcpStream) {
    loop {
        let mut read = [0; 1028];
        match stream.read(&mut read).await {
            Ok(n) => {
                stream.write_all(&read[0..n]).await.unwrap();
            }
            Err(err) => panic!("{err}"),
        }
    }
}

/// Drive one client connection, logging its traffic into the shared file.
async fn run_connection(id: u8) {
    // Each connection builds its own logger over the *same* file. `FileLogger::open` opens the path
    // in append mode, which is what lets several loggers write to it concurrently without
    // overwriting each other, and the prefix is what tells their lines apart afterwards.
    let logger = FileLogger::open(LOG_PATH)
        .unwrap()
        .with_prefix(format!("[conn {id}] "));

    let mut client = LoggedStream::new(
        net::TcpStream::connect(SERVER_ADDRESS).await.unwrap(),
        LowercaseHexadecimalFormatter::new_default(),
        DefaultFilter,
        logger,
    );

    // `Open` is the one record kind that is never emitted automatically — it is a marker you record
    // yourself, which is handy here to mark where each connection begins.
    client.log_open(format!("Established connection {id}"));

    for round in 0..3u8 {
        // The first byte identifies the connection, so each line in the log is traceable back to
        // the task that wrote it even without looking at the prefix.
        let send = [id, round, 0xab, 0xcd];
        client.write_all(&send).await.unwrap();
        let mut response = [0u8; 4];
        client.read_exact(&mut response).await.unwrap();
    }

    // Dropping `client` at the end of this function emits the `Drop` record ("Deallocated.")
    // through the same logger, so the connection's last line is written here too.
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    // The loggers append rather than truncate, so a file left over from a previous run would keep
    // accumulating. Start from a clean slate.
    let _ = fs::remove_file(LOG_PATH);

    let listener = net::TcpListener::bind(SERVER_ADDRESS).await.unwrap();

    tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    tokio::spawn(handle_connection(stream));
                }
                Err(err) => panic!("{err}"),
            }
        }
    });

    // Run every connection at the same time, so their records genuinely interleave in the file
    // instead of arriving one batch after another.
    let mut connections = Vec::new();
    for id in 1..=CONNECTIONS {
        connections.push(tokio::spawn(run_connection(id)));
    }
    for connection in connections {
        connection.await.unwrap();
    }

    // Lines from different connections are interleaved in time, but each one is intact and carries
    // the prefix of the connection that produced it.
    println!("--- {LOG_PATH} ---");
    print!("{}", fs::read_to_string(LOG_PATH).unwrap());
}
