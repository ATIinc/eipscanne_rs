//! One connection served over an in-memory stream.

use std::io::Cursor;

use binrw::{BinRead, BinWrite};
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream, duplex};
use tokio::task::JoinHandle;

use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::ResponseStatusCode;
use eipscanne_rs::cip::types::CipUdint;
use eipscanne_rs::eip::command::EncapsStatusCode;
use eipscanne_rs::eip::packet::{EnIpPacket, EncapsulationHeader};
use eipscanne_rs::object_assembly::RequestObjectAssembly;

use adapter::error::Result;
use adapter::handler::Handler;
use adapter::packet::read_packet;
use adapter::server::serve_stream;

const SESSION_HANDLE: CipUdint = 0x10;

/// Answers every Get with one byte
struct Device;

impl Handler for Device {
    fn handle_get_attribute(
        &self,
        _class: u16,
        _instance: u16,
        _attribute: Option<u16>,
    ) -> std::result::Result<CipDataOpt, ResponseStatusCode> {
        Ok(CipDataOpt::Raw(vec![0x2a]))
    }

    fn handle_set_attribute(
        &self,
        _class: u16,
        _instance: u16,
        _attribute: Option<u16>,
        _data: &[u8],
    ) -> std::result::Result<(), ResponseStatusCode> {
        Ok(())
    }
}

/// Serves `Device` on one end of an in-memory stream and returns the other
fn connect() -> (DuplexStream, JoinHandle<Result<()>>) {
    let (scanner, adapter) = duplex(1024);
    let connection = tokio::spawn(serve_stream(adapter, SESSION_HANDLE, &Device));
    (scanner, connection)
}

/// Writes `packet` to `stream`
async fn send(stream: &mut DuplexStream, packet: &EnIpPacket) {
    let mut bytes = Cursor::new(Vec::new());
    packet
        .write(&mut bytes)
        .expect("encoding into a Vec cannot fail");
    stream
        .write_all(bytes.get_ref())
        .await
        .expect("the adapter keeps the stream open until Unregister Session");
}

#[tokio::test]
async fn register_get_unregister() {
    let (mut stream, connection) = connect();

    send(&mut stream, &RequestObjectAssembly::new_registration()).await;
    let reply = read_packet(&mut stream)
        .await
        .expect("the adapter answers Register Session");
    assert_eq!(reply.header.session_handle, SESSION_HANDLE);

    send(
        &mut stream,
        &RequestObjectAssembly::new_identity(SESSION_HANDLE),
    )
    .await;
    let reply = read_packet(&mut stream)
        .await
        .expect("the adapter answers Send RR Data");
    assert!(
        reply
            .response()
            .expect("a Send RR Data reply carries a Message Router response")
            .is_success()
    );

    send(
        &mut stream,
        &RequestObjectAssembly::new_unregistration(SESSION_HANDLE),
    )
    .await;
    connection
        .await
        .expect("serving a stream does not panic")
        .expect("Unregister Session ends the stream cleanly");
}

#[tokio::test]
async fn a_refused_packet_keeps_the_stream() {
    let (mut stream, _connection) = connect();

    // A request before Register Session
    send(
        &mut stream,
        &RequestObjectAssembly::new_identity(SESSION_HANDLE),
    )
    .await;
    let mut bytes = [0u8; 24];
    stream
        .read_exact(&mut bytes)
        .await
        .expect("the adapter answers a refused packet with a bare header");
    let header = EncapsulationHeader::read(&mut Cursor::new(&bytes))
        .expect("any 24 bytes parse as a header");
    assert_eq!(header.status_code, EncapsStatusCode::InvalidSessionHandle);

    send(&mut stream, &RequestObjectAssembly::new_registration()).await;
    let reply = read_packet(&mut stream)
        .await
        .expect("the adapter answers Register Session");
    assert_eq!(reply.header.status_code, EncapsStatusCode::Success);
}
