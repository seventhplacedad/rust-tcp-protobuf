use core::panic;
use std::os::linux::raw;
use log::{debug, error, info, trace, warn};
use simplelog::*;
use std::fs::File;
use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;
use std::os::unix::prelude::AsRawFd;

#[derive(Debug)]
enum InvalidHeaderReason {
    BadMagic(u8),
    TooLongPDU(usize),
}
enum PDUReadOk {
    ReadPDU(Vec<u8>),
    NoPDU,
}

#[derive(Debug)]
enum PDUReadErr {
    InvalidHeaderForPDU(InvalidHeaderReason),
    GotEOF,
    DataReadWouldBlocked,
    HeaderIoError(std::io::Error),
    DataIoError(std::io::Error),
}

fn pdu(stream: &mut TcpStream) -> Result<PDUReadOk, PDUReadErr> {
    let mut buf = [0u8; 3];
    const MAGIC: u8 = 97u8;
    const MAX_DATA_SIZE: usize = 16000;

    match stream.read_exact(&mut buf) {
        Ok(()) => {
            let ascii_bytes = &buf[1..3];
            let my_hex_str : String = String::from_utf8(ascii_bytes.to_vec()).unwrap();
            assert!(my_hex_str.len() == 2);
            let raw_bytes : Vec<u8> = hex::decode(my_hex_str).unwrap();
            assert!(raw_bytes.len() == 1);
            let byte_count = u8::from_be_bytes(raw_bytes.try_into().unwrap()) as usize;
            match (buf[0], byte_count) {
                (MAGIC, count) if count < MAX_DATA_SIZE => {
                    let mut databuf = vec![0;byte_count];
                    match stream.read_exact(&mut databuf) {
                        Ok(_) => Ok(PDUReadOk::ReadPDU(databuf)),
                        Err(ref error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            Err(PDUReadErr::DataReadWouldBlocked)
                        }
                        Err(ref error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                            Err(PDUReadErr::GotEOF)
                        }
                        Err(err) => Err(PDUReadErr::DataIoError(err)),
                    }
                }
                (MAGIC, large_data_count) => Err(PDUReadErr::InvalidHeaderForPDU(
                    InvalidHeaderReason::TooLongPDU(large_data_count),
                )),
                (bad_magic, _) => Err(PDUReadErr::InvalidHeaderForPDU(
                    InvalidHeaderReason::BadMagic(bad_magic),
                )),
            }
        }

        Err(ref error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(PDUReadOk::NoPDU),
        Err(ref error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
            Err(PDUReadErr::GotEOF)
        }
        Err(err) => Err(PDUReadErr::HeaderIoError(err)),
    }
}

fn main() {
    CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Trace,
            Config::default(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            LevelFilter::Trace,
            Config::default(),
            File::create("myserver.log").unwrap(),
        ),
    ])
    .unwrap();

    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut streams: Vec<TcpStream> = Vec::new();

    info!("Server is initialized.");
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(true).unwrap();
                let fdnum: i32 = stream.as_raw_fd();
                info!("New Client fd={} Connected, added to stream list. ", fdnum);

                streams.push(stream);
            }
            Err(ref error) if error.kind() == std::io::ErrorKind::WouldBlock => (),
            Err(_) => panic!("Unexpected Error"),
        }

        let mut to_remove_indecies: Vec<usize> = Vec::new();
        for (i, stream) in streams.iter_mut().enumerate() {
            let fd = stream.as_raw_fd();
            match pdu(stream) {
                Err(err) => {
                    match err {
                        PDUReadErr::GotEOF => warn!(
                            "Client fd={} gave an EOF when reading possible PDU. Removing.",
                            fd
                        ),
                        other_error => error!(
                            "Reading PDU from client fd={} gave unexpected error. {:?}",
                            fd, other_error
                        ),
                    }
                    to_remove_indecies.push(i);
                }
                Ok(PDUReadOk::ReadPDU(woof)) => info!("I read a PDU mom {:?}", woof),
                Ok(PDUReadOk::NoPDU) => (),
            }
        }

        to_remove_indecies.sort();
        let reversed_remove_indecies: Vec<usize> = to_remove_indecies.into_iter().rev().collect();

        for i in reversed_remove_indecies {
            let stream = streams.remove(i);
            let fd = stream.as_raw_fd();
            stream.shutdown(std::net::Shutdown::Both).unwrap();
            info!("Shutdown stream fd={}", fd);
        }
    }
}
