use core::panic;
use log::{debug, error, info, trace, warn};
use simplelog::*;
use std::fs::File;
use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;

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

use rust_tcp_protobuf::protos::my_messages;
use protobuf::{parse_from_bytes,Message};
use rust_tcp_protobuf::buster;

fn maybe_get_pdu(stream: &mut TcpStream) -> Result<PDUReadOk, PDUReadErr> {
    let mut buf = [0u8; 5];
    const MAGIC: u8 = 97u8;
    const MAX_DATA_SIZE: usize = 16000;

    match stream.read_exact(&mut buf) {
        Ok(()) => {
            let num_bytes = &buf[1..5];
            let byte_count = u32::from_be_bytes(num_bytes.try_into().unwrap()) as usize;
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

    let listener = TcpListener::bind("0.0.0.0:7878").unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut streams: Vec<TcpStream> = Vec::new();

    info!("Server is initialized.");
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(true).unwrap();
                info!("New Client Connected, added to stream list. ");

                streams.push(stream);
            }
            Err(ref error) if error.kind() == std::io::ErrorKind::WouldBlock => (),
            Err(_) => panic!("Unexpected Error"),
        }

        let mut to_remove_indecies: Vec<usize> = Vec::new();
        for (i, stream) in streams.iter_mut().enumerate() {
            match maybe_get_pdu(stream) {
                Err(err) => {
                    match err {
                        PDUReadErr::GotEOF => warn!(
                            "Client gave an EOF when reading possible PDU. Removing.",
                        ),
                        other_error => error!(
                            "Reading PDU from client gave unexpected error. {:?}",
                            other_error
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
            stream.shutdown(std::net::Shutdown::Both).unwrap();
            info!("Shutdown stream ");
        }
    }
}
