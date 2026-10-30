//! The Message Router of a device with implicit messaging: the device's Get and Set, opening and
//! closing the connections whose tasks exchange the I/O, and routing each O->T packet to its
//! connection.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddrV4};
use std::ops::RangeInclusive;
use std::sync::{Arc, Mutex};

use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio_util::task::AbortOnDropHandle;

use eipscanne_rs::cip::connection_manager::forward_close::{
    ForwardCloseRequest, ForwardCloseResponse,
};
use eipscanne_rs::cip::connection_manager::forward_open::{
    ForwardOpenRequest, ForwardOpenResponse,
};
use eipscanne_rs::cip::connection_manager::response::ConnectionManagerExtendedStatus;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::{Rejection, ResponseStatusCode};
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::types::CipUdint;
use eipscanne_rs::eip::constants::ETHERNET_IP_IO_UDP_PORT;
use eipscanne_rs::eip::socket_addr::{SocketAddrInfo, SocketAddrInfoItems};

use crate::error::Result;
use crate::implicit::connection::{OpenConnection, connection_from_forward_open};
use crate::implicit::o2t::{receive_outputs, route_outputs};
use crate::implicit::t2o::send_inputs;
use crate::implicit::{ImplicitDevice, ImplicitOptions};
use crate::message_router::{MessageRouter, connection_failure_rejection};

/// O->T packets waiting for their connection; more are lost, like on the network
const OUTPUT_PACKET_QUEUE: usize = 8;

/// The Message Router of an [`ImplicitDevice`]: its Get and Set, plus Forward_Open and
/// Forward_Close
pub(crate) struct ImplicitMessageRouter<D> {
    device: Arc<D>,
    /// The UDP socket of every connection's I/O
    io_socket: Arc<UdpSocket>,
    /// The O->T Socket Address Info of a granted Forward_Open: the I/O port, unless it is 2222
    o2t_socket_addr_info: Option<SocketAddrInfo>,
    /// The open connections by O->T network connection id; one has timed out once its I/O
    /// exchange task finishes
    open_connections: Arc<Mutex<HashMap<CipUdint, OpenConnection>>>,
    max_io_connections: usize,
    packet_interval: RangeInclusive<u32>,
    /// Aborts the routing task when the Message Router drops; every connection then closes
    _output_routing_task: AbortOnDropHandle<()>,
}

impl<D: ImplicitDevice> ImplicitMessageRouter<D> {
    /// The Message Router of `device`, its I/O socket bound on `ip`
    pub(crate) async fn bind(device: Arc<D>, ip: IpAddr, options: ImplicitOptions) -> Result<Self> {
        let io_socket = Arc::new(UdpSocket::bind((ip, options.io_port)).await?);
        let io_address = io_socket.local_addr()?;
        tracing::info!(io = %io_address, "implicit messaging");
        let open_connections = Arc::new(Mutex::new(HashMap::new()));
        let _output_routing_task = AbortOnDropHandle::new(tokio::spawn(route_outputs(
            Arc::clone(&io_socket),
            Arc::clone(&open_connections),
        )));
        Ok(ImplicitMessageRouter {
            device,
            io_socket,
            o2t_socket_addr_info: (io_address.port() != ETHERNET_IP_IO_UDP_PORT).then(|| {
                SocketAddrInfo::from(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, io_address.port()))
            }),
            open_connections,
            max_io_connections: options.max_io_connections,
            packet_interval: options.packet_interval,
            _output_routing_task,
        })
    }
}

impl<D: ImplicitDevice> MessageRouter for ImplicitMessageRouter<D> {
    // The device's Get and Set
    fn handle_get_attributes_all(
        &self,
        class: u16,
        instance: u16,
    ) -> Result<CipDataOpt, ResponseStatusCode> {
        self.device.handle_get_attributes_all(class, instance)
    }

    fn handle_get_attribute_single(
        &self,
        class: u16,
        instance: u16,
        attribute: u16,
    ) -> Result<CipDataOpt, ResponseStatusCode> {
        self.device
            .handle_get_attribute_single(class, instance, attribute)
    }

    fn handle_set_attributes_all(
        &self,
        class: u16,
        instance: u16,
        data: &[u8],
    ) -> Result<(), ResponseStatusCode> {
        self.device.handle_set_attributes_all(class, instance, data)
    }

    fn handle_set_attribute_single(
        &self,
        class: u16,
        instance: u16,
        attribute: u16,
        data: &[u8],
    ) -> Result<(), ResponseStatusCode> {
        self.device
            .handle_set_attribute_single(class, instance, attribute, data)
    }

    /// Opens the connection `forward_open_request` asks for when a place is free and no other
    /// connection owns its outputs
    fn handle_forward_open(
        &self,
        forward_open_request: ForwardOpenRequest,
        originator_ip: IpAddr,
        t2o_socket_addr_info: Option<SocketAddrInfo>,
    ) -> Result<(ForwardOpenResponse, SocketAddrInfoItems), Rejection> {
        let service = forward_open_request.service_code();
        let triad = forward_open_request.connection_triad;
        let mut open_connections = self
            .open_connections
            .lock()
            .expect("nothing panics while holding the connections lock");
        // Timed-out connections free their place
        open_connections
            .retain(|_, open_connection| !open_connection.output_packet_sender.is_closed());
        // A duplicate, or one too many, is refused before it is checked
        if open_connections
            .values()
            .any(|open_connection| open_connection.connection_triad == triad)
        {
            return Err(connection_failure_rejection(
                service,
                ConnectionManagerExtendedStatus::ConnectionInUseOrDuplicateForwardOpen,
            ));
        }
        if open_connections.len() >= self.max_io_connections {
            return Err(connection_failure_rejection(
                service,
                ConnectionManagerExtendedStatus::OutOfConnections,
            ));
        }

        // Checks its parameters, path and sizes against the device
        let mut connection = connection_from_forward_open(
            self.device.as_ref(),
            forward_open_request,
            &self.packet_interval,
            originator_ip,
            t2o_socket_addr_info,
        )?;
        // Outputs have one owner; heartbeats carry none
        if connection.o2t.size > 0
            && open_connections
                .values()
                .any(|open_connection| open_connection.output_instance == connection.o2t.instance)
        {
            return Err(connection_failure_rejection(
                service,
                ConnectionManagerExtendedStatus::OwnershipConflict,
            ));
        }
        // Each open connection has its own O->T id
        while open_connections.contains_key(&connection.response.o2t_network_connection_id) {
            connection.response.o2t_network_connection_id = rand::random();
        }
        let forward_open_response = connection.response.clone();
        tracing::info!(
            o2t_id = format_args!("{:#010x}", forward_open_response.o2t_network_connection_id),
            t2o_id = format_args!("{:#010x}", forward_open_response.t2o_network_connection_id),
            o2t_api_us = forward_open_response.o2t_actual_packet_interval,
            t2o_api_us = forward_open_response.t2o_actual_packet_interval,
            output_instance = connection.o2t.instance,
            input_instance = connection.t2o.instance,
            t2o_endpoint = %connection.t2o_endpoint,
            "class 1 connection opened"
        );
        let output_instance = connection.o2t.instance;
        let device = Arc::clone(&self.device);
        let io_socket = Arc::clone(&self.io_socket);
        let (output_packet_sender, mut output_packet_receiver) = mpsc::channel(OUTPUT_PACKET_QUEUE);
        // Exchanges the I/O until the outputs time out or the connection is closed
        tokio::spawn(async move {
            tokio::select! {
                () = receive_outputs(device.as_ref(), &connection, &mut output_packet_receiver) => {}
                () = send_inputs(device.as_ref(), &connection, &io_socket) => {}
            }
        });
        open_connections.insert(
            forward_open_response.o2t_network_connection_id,
            OpenConnection {
                connection_triad: triad,
                originator_ip,
                output_instance,
                output_packet_sender,
            },
        );
        Ok((
            forward_open_response,
            SocketAddrInfoItems {
                o2t: self.o2t_socket_addr_info,
                t2o: None,
            },
        ))
    }

    /// Closes the open connection `forward_close_request` names, when `originator_ip` opened it,
    /// and idles its outputs
    fn handle_forward_close(
        &self,
        forward_close_request: ForwardCloseRequest,
        originator_ip: IpAddr,
    ) -> Result<ForwardCloseResponse, Rejection> {
        let service = ServiceCode::ForwardClose;
        let triad = forward_close_request.connection_triad;
        let mut open_connections = self
            .open_connections
            .lock()
            .expect("nothing panics while holding the connections lock");
        // A timed-out connection is not found
        let Some((&o2t_network_connection_id, current)) =
            open_connections.iter().find(|(_, open_connection)| {
                open_connection.connection_triad == triad
                    && !open_connection.output_packet_sender.is_closed()
            })
        else {
            return Err(connection_failure_rejection(
                service,
                ConnectionManagerExtendedStatus::TargetConnectionNotFound,
            ));
        };
        if current.originator_ip != originator_ip {
            return Err(Rejection {
                service,
                general_status: ResponseStatusCode::PrivilegeViolation,
                additional_status: vec![],
            });
        }
        // Dropping its sender makes the I/O exchange task idle the outputs and end
        open_connections.remove(&o2t_network_connection_id);
        tracing::info!(
            connection_serial_number = triad.connection_serial_number,
            "class 1 connection closed"
        );
        Ok(ForwardCloseResponse {
            connection_triad: triad,
            application_reply_size: 0,
            application_reply: vec![],
        })
    }
}
