//! A small device served on the loopback interface, driven by the scanner crate and by raw bytes.

use std::io::Cursor;
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};

use binrw::{BinRead, BinWrite};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use eipscanne_rs::cip::identity::{
    DeviceType, IdentityResponse, IdentityStatusBits, Revision, VendorId,
};
use eipscanne_rs::cip::message::data::CipData;
use eipscanne_rs::cip::message::response::ResponseStatusCode;
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::object_ids::{IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID};
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::{CipShortString, CipUdint};
use eipscanne_rs::eip::command::{EnIpCommand, EncapsStatusCode};
use eipscanne_rs::eip::packet::{EnIpPacket, EncapsulationHeader};
use eipscanne_rs::object_assembly::RequestObjectAssembly;

use adapter::handler::{Handler, decode};
use adapter::packet::read_packet;
use adapter::server::serve;
use scanner::error::Error;
use scanner::explicit::{decode_reply, read_identity, send_request};
use scanner::session::{Session, SessionConfig};

const ASSEMBLY_CLASS: u16 = 0x04;
const OUTPUT_INSTANCE: u16 = 0x64;
const DATA_ATTRIBUTE: u16 = 0x03;
const PRODUCT_NAME: &str = "Test Device";

/// The data of the output assembly
#[derive(BinRead, BinWrite, Debug, PartialEq, Clone, Copy)]
#[brw(little)]
struct Outputs {
    word: u32,
}

/// An Identity object and one output assembly
struct TestDevice {
    outputs: Mutex<Outputs>,
}

impl Handler for TestDevice {
    fn get(
        &self,
        class: u16,
        instance: u16,
        attribute: Option<u16>,
    ) -> Result<Box<dyn CipData>, ResponseStatusCode> {
        match (class, instance, attribute) {
            (IDENTITY_CLASS_ID, IDENTITY_INSTANCE_ID, None) => Ok(Box::new(IdentityResponse {
                vendor_id: VendorId::TeknicInc,
                device_type: DeviceType::GenericDevice,
                product_code: 1,
                revision: Revision { major: 1, minor: 2 },
                status: IdentityStatusBits::default().into(),
                serial_number: 0x0102_0304,
                product_name: CipShortString::from(PRODUCT_NAME.to_string()),
            })),
            (ASSEMBLY_CLASS, OUTPUT_INSTANCE, Some(DATA_ATTRIBUTE)) => {
                Ok(Box::new(*self.outputs.lock().unwrap()))
            }
            _ => Err(ResponseStatusCode::ObjectDoesNotExist),
        }
    }

    fn set(
        &self,
        class: u16,
        instance: u16,
        attribute: Option<u16>,
        data: &[u8],
    ) -> Result<(), ResponseStatusCode> {
        match (class, instance, attribute) {
            (ASSEMBLY_CLASS, OUTPUT_INSTANCE, Some(DATA_ATTRIBUTE)) => {
                *self.outputs.lock().unwrap() = decode(data)?;
                Ok(())
            }
            _ => Err(ResponseStatusCode::ObjectDoesNotExist),
        }
    }
}

/// Serves a fresh device on an ephemeral port and returns the port
async fn start_adapter() -> u16 {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let device = TestDevice {
        outputs: Mutex::new(Outputs { word: 0 }),
    };
    tokio::spawn(serve(listener, Arc::new(device)));
    port
}

async fn register(port: u16) -> Session {
    Session::register((Ipv4Addr::LOCALHOST, port), SessionConfig::default())
        .await
        .unwrap()
}

/// The General Status a refused request was answered with
fn general_status(result: scanner::error::Result<EnIpPacket>) -> ResponseStatusCode {
    match result {
        Err(Error::Rejected(rejection)) => rejection.general_status,
        other => panic!("expected a rejection, got {other:?}"),
    }
}

/// The bytes of `packet` on the wire
fn bytes_of(packet: &EnIpPacket) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    packet.write(&mut bytes).unwrap();
    bytes.into_inner()
}

fn output_path() -> CipPath {
    CipPath::new_full(
        ASSEMBLY_CLASS as u8,
        OUTPUT_INSTANCE as u8,
        DATA_ATTRIBUTE as u8,
    )
}

#[tokio::test]
async fn get_and_set_through_the_scanner() {
    let mut session = register(start_adapter().await).await;

    let identity = read_identity(&mut session).await.unwrap();
    assert_eq!(
        identity.product_name,
        CipShortString::from(PRODUCT_NAME.to_string())
    );

    send_request(
        &mut session,
        output_path(),
        ServiceCode::SetAttributeSingle,
        Some(Box::new(Outputs { word: 0xdead_beef })),
    )
    .await
    .unwrap();

    let reply = send_request(
        &mut session,
        output_path(),
        ServiceCode::GetAttributeSingle,
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        decode_reply::<Outputs>(&reply).unwrap(),
        Outputs { word: 0xdead_beef }
    );

    session.unregister().await.unwrap();
}

#[tokio::test]
async fn refusals_are_general_statuses() {
    let mut session = register(start_adapter().await).await;

    // The handler's refusal
    let unknown = CipPath::new_full(ASSEMBLY_CLASS as u8, 0x65, DATA_ATTRIBUTE as u8);
    let result = send_request(&mut session, unknown, ServiceCode::GetAttributeSingle, None).await;
    assert_eq!(
        general_status(result),
        ResponseStatusCode::ObjectDoesNotExist
    );

    // `decode`'s refusals
    let result = send_request(
        &mut session,
        output_path(),
        ServiceCode::SetAttributeSingle,
        Some(Box::new(0xbeef_u16)),
    )
    .await;
    assert_eq!(general_status(result), ResponseStatusCode::NotEnoughData);
    let result = send_request(
        &mut session,
        output_path(),
        ServiceCode::SetAttributeSingle,
        Some(Box::new(0xdead_beef_u64)),
    )
    .await;
    assert_eq!(general_status(result), ResponseStatusCode::TooMuchData);

    // A Single service without an attribute
    let result = send_request(
        &mut session,
        CipPath::new(ASSEMBLY_CLASS, OUTPUT_INSTANCE),
        ServiceCode::GetAttributeSingle,
        None,
    )
    .await;
    assert_eq!(
        general_status(result),
        ResponseStatusCode::PathDestinationUnknown
    );

    // A path that names no attribute: a connection path
    let result = send_request(
        &mut session,
        CipPath::new_assembly_connection(0x01, 0x64, 0x65),
        ServiceCode::GetAttributeAll,
        None,
    )
    .await;
    assert_eq!(general_status(result), ResponseStatusCode::PathSegmentError);

    // A service other than Get and Set
    let result = send_request(&mut session, output_path(), ServiceCode::Reset, None).await;
    assert_eq!(
        general_status(result),
        ResponseStatusCode::ServiceNotSupported
    );

    // The session survives every refusal
    read_identity(&mut session).await.unwrap();
}

#[tokio::test]
async fn each_connection_has_its_own_session() {
    let port = start_adapter().await;
    let mut first = register(port).await;
    let mut second = register(port).await;
    assert_ne!(first.session_handle(), second.session_handle());

    read_identity(&mut first).await.unwrap();
    read_identity(&mut second).await.unwrap();
}

#[tokio::test]
async fn packets_are_read_whole_however_they_arrive() {
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, start_adapter().await))
        .await
        .unwrap();

    // Register Session split in two writes
    let register = bytes_of(&RequestObjectAssembly::new_registration());
    stream.write_all(&register[..10]).await.unwrap();
    stream.flush().await.unwrap();
    stream.write_all(&register[10..]).await.unwrap();
    let session_handle = read_packet(&mut stream)
        .await
        .unwrap()
        .header
        .session_handle;

    // Two requests in one write
    let mut requests = bytes_of(&RequestObjectAssembly::new_identity(session_handle));
    requests.extend(bytes_of(&RequestObjectAssembly::new_identity(
        session_handle,
    )));
    stream.write_all(&requests).await.unwrap();
    for _ in 0..2 {
        let reply = read_packet(&mut stream).await.unwrap();
        assert!(reply.response().unwrap().is_success());
    }
}

/// The header of a reply that only carries an encapsulation status
async fn read_status_reply(stream: &mut TcpStream) -> EncapsulationHeader {
    let mut bytes = [0u8; 24];
    stream.read_exact(&mut bytes).await.unwrap();
    let header = EncapsulationHeader::read(&mut Cursor::new(&bytes)).unwrap();
    assert_eq!(header.length, Some(0));
    header
}

#[tokio::test]
async fn encapsulation_refusals_keep_the_connection() {
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, start_adapter().await))
        .await
        .unwrap();

    // A request before Register Session
    let unregistered: CipUdint = 0x1234;
    stream
        .write_all(&bytes_of(&RequestObjectAssembly::new_identity(
            unregistered,
        )))
        .await
        .unwrap();
    let header = read_status_reply(&mut stream).await;
    assert_eq!(header.command, EnIpCommand::SendRrData);
    assert_eq!(header.status_code, EncapsStatusCode::InvalidSessionHandle);

    // A command the adapter does not serve: List Identity, a bare header
    let mut list_identity = bytes_of(&RequestObjectAssembly::new_registration());
    list_identity[..2].copy_from_slice(&0x0063_u16.to_le_bytes());
    list_identity[2..4].copy_from_slice(&0_u16.to_le_bytes());
    list_identity.truncate(24);
    stream.write_all(&list_identity).await.unwrap();
    let header = read_status_reply(&mut stream).await;
    assert_eq!(header.command, EnIpCommand::ListIdentity);
    assert_eq!(header.status_code, EncapsStatusCode::UnsupportedCommand);

    // Still served
    stream
        .write_all(&bytes_of(&RequestObjectAssembly::new_registration()))
        .await
        .unwrap();
    let reply = read_packet(&mut stream).await.unwrap();
    assert_eq!(reply.header.status_code, EncapsStatusCode::Success);
}
