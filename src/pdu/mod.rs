use std::io::prelude::*;
use std::net::TcpStream;

#[derive(Debug)]
pub enum InvalidHeaderReason {
    BadMagic(u8),
    TooLongPDU(usize),
}
pub enum PDUReadOk {
    ReadPDU(Vec<u8>),
    NoPDU,
}

#[derive(Debug)]
pub enum PDUReadErr {
    InvalidHeaderForPDU(InvalidHeaderReason),
    GotEOF,
    DataReadWouldBlocked,
    HeaderIoError(std::io::Error),
    DataIoError(std::io::Error),
}

pub fn maybe_get_pdu(stream: &mut TcpStream) -> Result<PDUReadOk, PDUReadErr> {
    let mut buf = [0u8; 5];
    const MAGIC: u8 = 97u8;
    const MAX_DATA_SIZE: usize = 16000;

    match stream.read_exact(&mut buf) {
        Ok(()) => {
            let num_bytes = &buf[1..5];
            let byte_count = u32::from_be_bytes(num_bytes.try_into().unwrap()) as usize;
            match (buf[0], byte_count) {
                (MAGIC, count) if count < MAX_DATA_SIZE => {
                    let mut databuf = vec![0; byte_count];
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
