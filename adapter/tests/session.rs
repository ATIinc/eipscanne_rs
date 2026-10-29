//! One encapsulation session on the loopback interface.

use std::io::Cursor;
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;

use binrw::BinRead;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::Instant;

use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::ResponseStatusCode;
use eipscanne_rs::cip::types::CipUdint;
use eipscanne_rs::eip::command::{CommandSpecificData, EnIpCommand, EncapsStatusCode};
use eipscanne_rs::eip::constants::{
    DEFAULT_ENCAPSULATION_OPTIONS, EMPTY_SENDER_CONTEXT, UNREGISTERED_SESSION_HANDLE,
};
use eipscanne_rs::eip::packet::{EnIpPacket, EncapsulationHeader};
use eipscanne_rs::object_assembly::RequestObjectAssembly;

use adapter::message_router::{ExplicitOptions, MessageRouter};
use adapter::packet::{read_packet, write_packet};
use adapter::server::serve;

/// The handle of the first session a server grants
const SESSION_HANDLE: CipUdint = 1;

/// Answers Get_Attributes_All with one byte
struct SampleDevice;

impl MessageRouter for SampleDevice {
    fn handle_get_attributes_all(
        &self,
        _class: u16,
        _instance: u16,
    ) -> std::result::Result<CipDataOpt, ResponseStatusCode> {
        Ok(CipDataOpt::Raw(vec![0x2a]))
    }
}

/// Serves `SampleDevice` on the loopback interface and connects to it
async fn connect() -> TcpStream {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("port 0 lets the OS pick a free loopback port");
    let address = listener
        .local_addr()
        .expect("a bound listener has a local address");
    tokio::spawn(serve(
        Arc::new(SampleDevice),
        listener,
        ExplicitOptions::default(),
    ));
    TcpStream::connect(address)
        .await
        .expect("the server accepts on its bound port")
}

/// A `command` packet without data
fn no_data(command: EnIpCommand) -> EnIpPacket {
    EnIpPacket {
        header: EncapsulationHeader {
            command,
            length: None,
            session_handle: UNREGISTERED_SESSION_HANDLE,
            status_code: EncapsStatusCode::Success,
            sender_context: EMPTY_SENDER_CONTEXT,
            options: DEFAULT_ENCAPSULATION_OPTIONS,
        },
        command_specific_data: CommandSpecificData::NoData,
    }
}

#[tokio::test]
async fn register_get_unregister() {
    let mut stream = connect().await;

    write_packet(&mut stream, &RequestObjectAssembly::new_registration())
        .await
        .expect("the adapter keeps the stream open until Unregister Session");
    let reply = read_packet(&mut stream)
        .await
        .expect("the adapter answers Register Session");
    assert_eq!(reply.header.session_handle, SESSION_HANDLE);

    write_packet(
        &mut stream,
        &RequestObjectAssembly::new_identity(SESSION_HANDLE),
    )
    .await
    .expect("the adapter keeps the stream open until Unregister Session");
    let reply = read_packet(&mut stream)
        .await
        .expect("the adapter answers Send RR Data");
    assert!(
        reply
            .response()
            .expect("a Send RR Data reply carries a Message Router response")
            .is_success()
    );

    write_packet(
        &mut stream,
        &RequestObjectAssembly::new_unregistration(SESSION_HANDLE),
    )
    .await
    .expect("the adapter keeps the stream open until Unregister Session");
    let read = stream
        .read(&mut [0u8; 1])
        .await
        .expect("Unregister Session ends the stream cleanly");
    assert_eq!(read, 0, "the adapter closes the stream");
}

#[tokio::test]
async fn a_refused_packet_keeps_the_stream() {
    let mut stream = connect().await;

    // A request before Register Session
    write_packet(
        &mut stream,
        &RequestObjectAssembly::new_identity(SESSION_HANDLE),
    )
    .await
    .expect("the adapter keeps the stream open until Unregister Session");
    let mut bytes = [0u8; 24];
    stream
        .read_exact(&mut bytes)
        .await
        .expect("the adapter answers a refused packet with a bare header");
    let header = EncapsulationHeader::read(&mut Cursor::new(&bytes))
        .expect("any 24 bytes parse as a header");
    assert_eq!(header.status_code, EncapsStatusCode::InvalidSessionHandle);

    write_packet(&mut stream, &RequestObjectAssembly::new_registration())
        .await
        .expect("the adapter keeps the stream open until Unregister Session");
    let reply = read_packet(&mut stream)
        .await
        .expect("the adapter answers Register Session");
    assert_eq!(reply.header.status_code, EncapsStatusCode::Success);
}

#[tokio::test]
async fn nop_and_a_request_with_a_status_have_no_reply() {
    let mut stream = connect().await;

    let mut request_with_status = RequestObjectAssembly::new_identity(SESSION_HANDLE);
    request_with_status.header.status_code = EncapsStatusCode::InsufficientMemory;
    for packet in [
        no_data(EnIpCommand::NOP),
        request_with_status,
        RequestObjectAssembly::new_registration(),
    ] {
        write_packet(&mut stream, &packet)
            .await
            .expect("the adapter keeps the stream open until Unregister Session");
    }

    let reply = read_packet(&mut stream)
        .await
        .expect("the adapter answers Register Session");
    assert_eq!(reply.header.command, EnIpCommand::RegisterSession);
    assert_eq!(reply.header.status_code, EncapsStatusCode::Success);
}

#[tokio::test(start_paused = true)]
async fn a_session_without_encapsulation_activity_closes() {
    let mut stream = connect().await;
    let start = Instant::now();

    // A NOP is encapsulation activity
    tokio::time::sleep(Duration::from_secs(100)).await;
    write_packet(&mut stream, &no_data(EnIpCommand::NOP))
        .await
        .expect("the adapter keeps the stream open within the inactivity timeout");

    let read = stream
        .read(&mut [0u8; 1])
        .await
        .expect("the inactivity timeout ends the stream cleanly");
    assert_eq!(read, 0, "the adapter closes the stream");
    assert!(
        start.elapsed() >= Duration::from_secs(220),
        "the default 120 s count from the NOP"
    );
}
