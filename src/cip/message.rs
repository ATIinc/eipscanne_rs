pub mod data;
pub mod request;
pub mod response;
pub mod shared;

use std::io::{Read, Seek, SeekFrom, Write};

use binrw::{BinRead, BinResult, BinWrite, Endian};

use request::MessageRouterRequest;
use response::MessageRouterResponse;
use shared::ServiceContainer;

/// A Message Router message: a request or a response (Wireshark: "Common Industrial Protocol").
///
/// Which one is read is decided by the message itself: the top bit of its first byte (the service
/// code byte) is the Request/Response flag.
#[derive(Debug, PartialEq)]
pub enum CipMessage {
    Request(MessageRouterRequest),
    Response(MessageRouterResponse),
}

// ======= Start of CipMessage impl ========

impl From<MessageRouterRequest> for CipMessage {
    fn from(request: MessageRouterRequest) -> Self {
        CipMessage::Request(request)
    }
}

impl From<MessageRouterResponse> for CipMessage {
    fn from(response: MessageRouterResponse) -> Self {
        CipMessage::Response(response)
    }
}

impl BinRead for CipMessage {
    // The length of the message, which it does not encode itself
    type Args<'a> = (u16,);

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: Endian,
        args: Self::Args<'_>,
    ) -> BinResult<Self> {
        // Peek at the service code byte for the Request/Response flag, then read the whole message
        let service_container = ServiceContainer::from(u8::read_options(reader, endian, ())?);
        reader.seek(SeekFrom::Current(-1))?;

        if service_container.response() {
            MessageRouterResponse::read_options(reader, endian, args).map(CipMessage::Response)
        } else {
            MessageRouterRequest::read_options(reader, endian, args).map(CipMessage::Request)
        }
    }
}

impl BinWrite for CipMessage {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: Endian,
        args: Self::Args<'_>,
    ) -> BinResult<()> {
        match self {
            CipMessage::Request(request) => request.write_options(writer, endian, args),
            CipMessage::Response(response) => response.write_options(writer, endian, args),
        }
    }
}

// ^^^^^^^^ End of CipMessage impl ^^^^^^^^
