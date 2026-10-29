//! The Message Router of a device with implicit messaging: the device's Get and Set, and opening
//! and closing the one connection whose task exchanges the I/O.

use std::net::{IpAddr, Ipv4Addr, SocketAddrV4};
use std::sync::{Arc, Mutex};

use tokio::net::UdpSocket;
use tokio::task::AbortHandle;

use eipscanne_rs::cip::connection_manager::forward_close::{
    ForwardCloseRequest, ForwardCloseResponse,
};
use eipscanne_rs::cip::connection_manager::forward_open::ForwardOpenRequest;
use eipscanne_rs::cip::connection_manager::response::ConnectionManagerExtendedStatus;
use eipscanne_rs::cip::connection_manager::shared::ConnectionTriad;
use eipscanne_rs::cip::message::data::CipDataOpt;
use eipscanne_rs::cip::message::response::{MessageRouterResponse, Rejection, ResponseStatusCode};
use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::eip::constants::ETHERNET_IP_IO_UDP_PORT;
use eipscanne_rs::eip::socket_addr::{SocketAddrInfo, SocketAddrInfoItems};

use crate::error::Result;
use crate::implicit::ImplicitDevice;
use crate::implicit::connection::connection_from_forward_open;
use crate::implicit::o2t::receive_outputs;
use crate::implicit::t2o::send_inputs;
use crate::message_router::{
    MessageRouter, connection_failure, message_router_response, rejection_response,
};

/// The Message Router of an [`ImplicitDevice`]: its Get and Set, plus Forward_Open and
/// Forward_Close
pub(crate) struct ImplicitMessageRouter<D> {
    device: Arc<D>,
    /// The UDP socket of every connection's I/O
    io_socket: Arc<UdpSocket>,
    /// The O->T Socket Address Info of a granted Forward_Open: the I/O port, unless it is 2222
    o2t_socket_addr_info: Option<SocketAddrInfo>,
    /// The last connection opened; it has timed out once its I/O exchange task finishes
    open_connection: Mutex<Option<OpenConnection>>,
}

impl<D: ImplicitDevice> ImplicitMessageRouter<D> {
    /// The Message Router of `device`, its I/O socket bound on `ip`
    pub(crate) async fn bind(device: Arc<D>, ip: IpAddr) -> Result<Self> {
        let io_socket = UdpSocket::bind((ip, device.io_port())).await?;
        let io_address = io_socket.local_addr()?;
        tracing::info!(io = %io_address, "implicit messaging");
        Ok(ImplicitMessageRouter {
            device,
            io_socket: Arc::new(io_socket),
            o2t_socket_addr_info: (io_address.port() != ETHERNET_IP_IO_UDP_PORT).then(|| {
                SocketAddrInfo::from(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, io_address.port()))
            }),
            open_connection: Mutex::new(None),
        })
    }
}

impl<D: ImplicitDevice> MessageRouter for ImplicitMessageRouter<D> {
    // The device's Get and Set, and whether it accepts class 3 connections
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

    fn accepts_explicit_connections(&self) -> bool {
        self.device.accepts_explicit_connections()
    }

    /// Opens the one connection `forward_open` asks for
    fn handle_forward_open(
        &self,
        forward_open: ForwardOpenRequest,
        originator_ip: IpAddr,
        t2o_socket_addr_info: Option<SocketAddrInfo>,
    ) -> (MessageRouterResponse, SocketAddrInfoItems) {
        let service = forward_open.service_code();
        let triad = forward_open.connection_triad;
        let mut open_connection = self
            .open_connection
            .lock()
            .expect("nothing panics while holding the connection lock");
        // While a connection runs, every Forward_Open is refused before it is checked
        if let Some(current) = open_connection.as_ref()
            && !current.io_exchange_task.is_finished()
        {
            let extended_status = if current.connection_triad == triad {
                ConnectionManagerExtendedStatus::ConnectionInUseOrDuplicateForwardOpen
            } else {
                ConnectionManagerExtendedStatus::OutOfConnections
            };
            return (
                rejection_response(connection_failure(service, extended_status), triad),
                SocketAddrInfoItems::empty(),
            );
        }

        // Checks the Forward_Open against the device's assemblies
        let connection = match connection_from_forward_open(
            self.device.as_ref(),
            forward_open,
            originator_ip,
            t2o_socket_addr_info,
        ) {
            Ok(connection) => connection,
            Err(rejection) => {
                return (
                    rejection_response(rejection, triad),
                    SocketAddrInfoItems::empty(),
                );
            }
        };
        let forward_open_response = connection.response.clone();
        tracing::info!(
            o2t_id = format_args!("{:#010x}", forward_open_response.o2t_network_connection_id),
            t2o_id = format_args!("{:#010x}", forward_open_response.t2o_network_connection_id),
            o2t_api_us = forward_open_response.o2t_actual_packet_interval,
            t2o_api_us = forward_open_response.t2o_actual_packet_interval,
            output_instance = connection.output_instance,
            input_instance = connection.input_instance,
            t2o_endpoint = %connection.t2o_endpoint,
            "class 1 connection opened"
        );
        let device = Arc::clone(&self.device);
        let io_socket = Arc::clone(&self.io_socket);
        // Exchanges the I/O until the outputs time out or the connection is closed
        let io_exchange_task = tokio::spawn(async move {
            tokio::select! {
                () = receive_outputs(device.as_ref(), &connection, &io_socket) => {}
                () = send_inputs(device.as_ref(), &connection, &io_socket) => {}
            }
        })
        .abort_handle();
        // Replaces a timed-out connection, if any
        *open_connection = Some(OpenConnection {
            connection_triad: triad,
            originator_ip,
            io_exchange_task,
        });
        (
            message_router_response(
                service,
                ResponseStatusCode::Success,
                vec![],
                CipDataOpt::Typed(Box::new(forward_open_response)),
            ),
            SocketAddrInfoItems {
                o2t: self.o2t_socket_addr_info,
                t2o: None,
            },
        )
    }

    /// Closes the open connection `forward_close` names, when `originator_ip` opened it
    fn handle_forward_close(
        &self,
        forward_close: ForwardCloseRequest,
        originator_ip: IpAddr,
    ) -> MessageRouterResponse {
        let service = ServiceCode::ForwardClose;
        let triad = forward_close.connection_triad;
        let mut open_connection = self
            .open_connection
            .lock()
            .expect("nothing panics while holding the connection lock");
        if let Some(current) = open_connection.as_ref()
            && current.connection_triad == triad
            && !current.io_exchange_task.is_finished()
        {
            if current.originator_ip != originator_ip {
                return rejection_response(
                    Rejection {
                        service,
                        general_status: ResponseStatusCode::PrivilegeViolation,
                        additional_status: vec![],
                    },
                    triad,
                );
            }
            // Dropping it stops the I/O exchange task
            *open_connection = None;
            tracing::info!(
                connection_serial_number = triad.connection_serial_number,
                "connection closed"
            );
            return message_router_response(
                service,
                ResponseStatusCode::Success,
                vec![],
                CipDataOpt::Typed(Box::new(ForwardCloseResponse {
                    connection_triad: triad,
                    application_reply_size: 0,
                    application_reply: vec![],
                })),
            );
        }
        // A timed-out connection is not found
        rejection_response(
            connection_failure(
                service,
                ConnectionManagerExtendedStatus::TargetConnectionNotFound,
            ),
            triad,
        )
    }
}

/// The open connection: its triad names it in a Forward_Close, from the IP address that opened it;
/// it is open while its I/O exchange task runs, and dropping it stops that task
struct OpenConnection {
    connection_triad: ConnectionTriad,
    originator_ip: IpAddr,
    io_exchange_task: AbortHandle,
}

impl Drop for OpenConnection {
    fn drop(&mut self) {
        self.io_exchange_task.abort();
    }
}

#[cfg(test)]
mod tests {
    use bilge::prelude::u4;

    use eipscanne_rs::cip::connection_manager::parameters::{PriorityTimeTick, RealTimeFormat};
    use eipscanne_rs::cip::path::CipPath;

    use super::*;

    /// Implicit messaging on any free port, without assemblies
    struct SampleDevice;

    impl MessageRouter for SampleDevice {}

    impl ImplicitDevice for SampleDevice {
        type Outputs = ();
        type Inputs = ();

        fn io_port(&self) -> u16 {
            0
        }

        fn assembly_layout(&self, _instance: u16) -> Option<(u16, RealTimeFormat)> {
            None
        }

        fn consume_outputs(&self, _instance: u16, _outputs: ()) {}

        fn produce_inputs(&self, _instance: u16) {}
    }

    /// The Forward_Close of the connection `connection_triad` names
    fn forward_close(connection_triad: ConnectionTriad) -> ForwardCloseRequest {
        ForwardCloseRequest {
            priority_time_tick: PriorityTimeTick::builder()
                .tick_time(u4::new(10))
                .priority(false)
                .build(),
            timeout_ticks: 5,
            connection_triad,
            connection_path: CipPath::new_assembly_connection(151, 150, 100),
        }
    }

    #[tokio::test]
    async fn only_the_ip_address_that_opened_a_connection_closes_it() {
        let message_router =
            ImplicitMessageRouter::bind(Arc::new(SampleDevice), IpAddr::from(Ipv4Addr::LOCALHOST))
                .await
                .expect("port 0 lets the OS pick a free loopback port");
        let triad = ConnectionTriad::default();
        let opened_from = IpAddr::from(Ipv4Addr::new(192, 168, 1, 10));
        *message_router
            .open_connection
            .lock()
            .expect("nothing panics while holding the connection lock") = Some(OpenConnection {
            connection_triad: triad,
            originator_ip: opened_from,
            io_exchange_task: tokio::spawn(std::future::pending::<()>()).abort_handle(),
        });

        let response = message_router.handle_forward_close(
            forward_close(triad),
            IpAddr::from(Ipv4Addr::new(192, 168, 1, 11)),
        );
        assert_eq!(
            response.response_data.status,
            ResponseStatusCode::PrivilegeViolation
        );
        let response = message_router.handle_forward_close(forward_close(triad), opened_from);
        assert_eq!(response.response_data.status, ResponseStatusCode::Success);
    }
}
