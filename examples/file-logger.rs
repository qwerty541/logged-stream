use log::LevelFilter;
use logged_stream::DefaultFilter;
use logged_stream::FileLogger;
use logged_stream::LoggedStream;
use logged_stream::LowercaseHexadecimalFormatter;
use std::fs;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net;

const SERVER_ADDRESS: &str = "127.0.0.1:8080";
const LOG_PATH: &str = "./examples/traffic.log";

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

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    env_logger::builder()
        .parse_default_env()
        .filter_level(LevelFilter::Debug)
        .default_format()
        .format_timestamp_millis()
        .init();

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

    let mut client = LoggedStream::new(
        net::TcpStream::connect(SERVER_ADDRESS).await.unwrap(),
        LowercaseHexadecimalFormatter::new_default(),
        DefaultFilter,
        // A single logger owns this file, so truncating it on every run is fine. To let several
        // loggers share one file — for example one per connection, each tagged with its own
        // `with_prefix` — construct them with `FileLogger::open`, which opens the file in append
        // mode so their lines cannot overwrite or interleave with each other.
        FileLogger::new(fs::File::create(LOG_PATH).unwrap()),
    );

    let send = [0x01, 0x02, 0x03, 0x04];
    client.write_all(&send).await.unwrap();
    let mut response = [0u8; 4];
    client.read_exact(&mut response).await.unwrap();

    let send = [0x05, 0x06, 0x07, 0x08];
    client.write_all(&send).await.unwrap();
    let mut response = [0u8; 4];
    client.read_exact(&mut response).await.unwrap();

    let send = [0x09, 0x0a, 0x0b, 0x0c];
    client.write_all(&send).await.unwrap();
    let mut response = [0u8; 4];
    client.read_exact(&mut response).await.unwrap();

    let send = [0x01, 0x02, 0x03, 0x04];
    client.write_all(&send).await.unwrap();
    let mut response = [0u8; 4];
    client.read_exact(&mut response).await.unwrap();

    // Dropping `client` at the end of this function emits the `Drop` record ("Deallocated.") through
    // the same logger, so the connection's last line is written here too.
    drop(client);

    // Write the log file to stdout
    let mut result = String::new();

    result.push_str("Log file contents:");
    result.push('\n');
    result.push_str(&format!("--- {LOG_PATH} ---"));
    result.push('\n');
    result.push_str(&fs::read_to_string(LOG_PATH).unwrap());

    log::debug!("{result}");
}
