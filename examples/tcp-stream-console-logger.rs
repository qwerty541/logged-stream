use logged_stream::ConsoleLogger;
use logged_stream::HexDecimalFormatter;
use logged_stream::LoggedStream;
use std::env;
use std::io::Read;
use std::io::Write;
use std::net;
use std::thread;

fn handle_connection(mut stream: net::TcpStream) {
    loop {
        let mut read = [0; 1028];
        match stream.read(&mut read) {
            Ok(n) => {
                stream.write_all(&read[0..n]).unwrap();
            }
            Err(err) => {
                panic!("{err}");
            }
        }
    }
}

fn main() {
    env::set_var("RUST_LOG", "debug");
    env_logger::init();

    let listener = net::TcpListener::bind("127.0.0.1:8080").unwrap();

    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    thread::spawn(move || handle_connection(stream));
                }
                Err(_) => println!("Error"),
            }
        }
    });

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
