use logged_stream::ConsoleLogger;
use logged_stream::HexDecimalFormatter;
use logged_stream::LoggedStream;
use std::env;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net;

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
    env::set_var("RUST_LOG", "debug");
    env_logger::init();

    let listener = net::TcpListener::bind("127.0.0.1:8080").await.unwrap();

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
        net::TcpStream::connect("127.0.0.1:8080").await.unwrap(),
        HexDecimalFormatter::new(None),
        ConsoleLogger::new_unchecked("debug"),
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
}
