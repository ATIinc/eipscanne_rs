mod ethernet_ip;

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

use binrw::{BinRead, BinWrite};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use eipscanne_rs::cip::message::shared::ServiceCode;
use eipscanne_rs::cip::path::CipPath;
use eipscanne_rs::cip::types::CipUdint;
use eipscanne_rs::object_assembly::{RequestObjectAssembly, ResponseObjectAssembly};

use ethernet_ip::clearlink_config::{ConfigAssemblyObject, MotorConfigData};
use ethernet_ip::clearlink_input::{InputAssemblyObject, MotorInputData};
use ethernet_ip::clearlink_output::{MotorOutputData, OutputAssemblyObject};
use ethernet_ip::consts::{
    ASSEMBLY_ATTRIBUTE_ID, ASSEMBLY_OBJECT_ID, CONFIG_ASSEMBLY_INSTANCE_ID, ETHERNET_IP_PORT,
    INPUT_ASSEMBLY_INSTANCE_ID, OUTPUT_ASSEMBLY_INSTANCE_ID, SOCKET_OPEN_TIMEOUT,
};

// ============================================================
// HOMING PARAMETERS — adjust these to reproduce the bug
// ============================================================

/// IP address of the ClearLink device
// const CLEARLINK_IP: Ipv4Addr = Ipv4Addr::LOCALHOST;
const CLEARLINK_IP: Ipv4Addr = Ipv4Addr::new(172, 31, 19, 14);

/// Which motor connector to home (0 = M0, 1 = M1, 2 = M2, 3 = M3)
const MOTOR_INDEX: u8 = 1;

/// I/O pin index for the home sensor.
/// Set to the connector number (0–12) for sensor-based homing.
/// Set to -1 for hard-stop homing (motor stalls against a physical stop).
const HOME_SENSOR_CONNECTOR: i8 = 6;

/// Jog velocity for the homing move in raw steps/s.
const HOMING_VELOCITY_STEPS: i32 = 2000;

/// Acceleration limit for the homing move in steps/s².
const HOMING_ACCELERATION_STEPS: u32 = 100;

/// MODIFY THIS TO REPRODUCE THE BUG — try values that are positive and negative
/// Deceleration limit for the homing move in steps/s².
const HOMING_DECELERATION_STEPS: i32 = -8;

/// How often (in milliseconds) to poll the input assembly while waiting
/// for a status flag to change.
const POLL_INTERVAL_MS: u64 = 10;

// ---- EipConnection ------------------------------------------
//
// Owns the TCP stream, EIP session, and the two assembly objects.
// All wire-level communication goes through this struct.

struct EipConnection {
    stream: TcpStream,
    session_id: CipUdint, // 0 until register() is called
    motor_index: u8,
    output_assembly: OutputAssemblyObject,
    config_assembly: ConfigAssemblyObject,
}

impl EipConnection {
    async fn connect(motor_index: u8) -> Self {
        let addr = SocketAddr::V4(SocketAddrV4::new(CLEARLINK_IP, ETHERNET_IP_PORT));
        tracing::info!("Connecting to ClearLink at {addr}...");
        let stream = tokio::time::timeout(SOCKET_OPEN_TIMEOUT, TcpStream::connect(addr))
            .await
            .expect("Timed out connecting to ClearLink")
            .expect("Failed to connect to ClearLink");
        tracing::info!("Connected");

        Self {
            stream,
            session_id: 0,
            motor_index,
            output_assembly: OutputAssemblyObject::default(),
            config_assembly: ConfigAssemblyObject::default(),
        }
    }

    async fn register(&mut self) {
        tracing::info!("Registering EtherNet/IP session...");
        self.write_raw(RequestObjectAssembly::new_registration())
            .await;

        let response = self
            .read_raw::<u8>()
            .await
            .expect("Failed to read registration response");
        self.session_id = response.packet_description.header.session_handle;
        tracing::info!("Session registered: {}", self.session_id);
    }

    async fn unregister(&mut self) {
        tracing::info!("Unregistering EtherNet/IP session...");
        self.write_raw(RequestObjectAssembly::new_unregistration(self.session_id))
            .await;
        tracing::info!("Session unregistered");
    }

    // ---- Motor slot accessors -----------------------------------

    fn get_motor_output_mut(&mut self) -> &mut MotorOutputData {
        self.output_assembly.get_motor_output_mut(self.motor_index)
    }

    fn get_motor_config_mut(&mut self) -> &mut MotorConfigData {
        self.config_assembly.get_motor_config_mut(self.motor_index)
    }

    // ---- Assembly writes ----------------------------------------

    async fn write_output_assembly(&mut self) {
        let assembly = self.output_assembly;
        self.write_raw(RequestObjectAssembly::new_service_request(
            self.session_id,
            CipPath::new_full(
                ASSEMBLY_OBJECT_ID,
                OUTPUT_ASSEMBLY_INSTANCE_ID,
                ASSEMBLY_ATTRIBUTE_ID,
            ),
            ServiceCode::SetAttributeSingle,
            Some(assembly),
        ))
        .await;
        let _ack = self
            .read_raw::<u8>()
            .await
            .expect("Failed to read output assembly write ack");
    }

    async fn write_config_assembly(&mut self) {
        let assembly = self.config_assembly;
        self.write_raw(RequestObjectAssembly::new_service_request(
            self.session_id,
            CipPath::new_full(
                ASSEMBLY_OBJECT_ID,
                CONFIG_ASSEMBLY_INSTANCE_ID,
                ASSEMBLY_ATTRIBUTE_ID,
            ),
            ServiceCode::SetAttributeSingle,
            Some(assembly),
        ))
        .await;
        let _ack = self
            .read_raw::<u8>()
            .await
            .expect("Failed to read config assembly write ack");
    }

    // ---- Input polling ------------------------------------------

    async fn poll_motor<C>(&mut self, condition: C)
    where
        C: Fn(&MotorInputData) -> bool,
    {
        loop {
            let assembly = self.read_input_assembly().await;
            let motor = assembly.motor_input(self.motor_index);
            if condition(motor) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
        }
    }

    async fn read_input_assembly(&mut self) -> InputAssemblyObject {
        self.write_raw(RequestObjectAssembly::<u8>::new_service_request(
            self.session_id,
            CipPath::new_full(
                ASSEMBLY_OBJECT_ID,
                INPUT_ASSEMBLY_INSTANCE_ID,
                ASSEMBLY_ATTRIBUTE_ID,
            ),
            ServiceCode::GetAttributeSingle,
            None,
        ))
        .await;

        let packet = self
            .read_raw::<InputAssemblyObject>()
            .await
            .expect("Failed to read input assembly");
        packet
            .cip_message
            .expect("No CIP message in input assembly response")
            .response_data
            .data
            .expect("No data in input assembly CIP message")
    }

    // ---- Raw I/O ------------------------------------------------

    async fn write_raw<T>(&mut self, object_assembly: RequestObjectAssembly<T>)
    where
        T: for<'a> BinWrite<Args<'a> = ()>,
    {
        let mut buf: Vec<u8> = Vec::new();
        object_assembly
            .write(&mut std::io::Cursor::new(&mut buf))
            .unwrap();

        self.stream
            .write_all(&buf)
            .await
            .expect("Failed to write to ClearLink");
    }

    async fn read_raw<T>(&mut self) -> Result<ResponseObjectAssembly<T>, binrw::Error>
    where
        T: for<'a> BinRead<Args<'a> = ()>,
    {
        let mut buf = vec![0; 1000];
        let n = self.stream.read(&mut buf).await?;
        buf.truncate(n);
        ResponseObjectAssembly::<T>::read(&mut std::io::Cursor::new(buf))
    }
}

// ---- HomingSession ------------------------------------------
//
// High-level homing sequence. Each method corresponds to one stage.
// State is held inside `connection`; methods mutate the relevant motor
// slot and flush the assembly to the device.

struct HomingSession {
    connection: EipConnection,
}

impl HomingSession {
    async fn new() -> Self {
        Self {
            connection: EipConnection::connect(MOTOR_INDEX).await,
        }
    }

    async fn register(&mut self) {
        self.connection.register().await;
    }

    async fn unregister(&mut self) {
        self.connection.unregister().await;
    }

    async fn write_homing_config(&mut self) {
        tracing::info!(
            "Writing config — motor {}, home_sensor_connector={HOME_SENSOR_CONNECTOR}, homing_enable=true",
            self.connection.motor_index
        );
        let cfg = self.connection.get_motor_config_mut();
        cfg.config_register.set_homing_enable(true);
        cfg.config_register.set_hlfb_inversion(true);
        cfg.home_sensor_connector = HOME_SENSOR_CONNECTOR;
        cfg.max_deceleration = HOMING_DECELERATION_STEPS;

        self.connection.write_config_assembly().await;
        // Allow the device time to process the config change before sending motion commands.
        tokio::time::sleep(Duration::from_millis(200)).await;
        tracing::info!("Config written");
    }

    async fn enable_motor(&mut self) {
        tracing::info!("Enabling motor...");
        self.connection
            .get_motor_output_mut()
            .output_register
            .set_enable(true);

        self.connection.write_output_assembly().await;
        self.connection
            .poll_motor(|m| m.motor_status.enabled())
            .await;
        tracing::info!("Motor enabled");
    }

    async fn wait_for_ready_to_home(&mut self) {
        tracing::info!("Waiting for ready_to_home...");
        self.connection
            .poll_motor(|m| m.motor_status.ready_to_home())
            .await;
        tracing::info!("Motor is ready to home");
    }

    async fn home(&mut self) {
        tracing::info!(
            "Issuing homing move — velocity={HOMING_VELOCITY_STEPS} steps/s, \
             accel={HOMING_ACCELERATION_STEPS} steps/s², decel={HOMING_DECELERATION_STEPS} steps/s²"
        );

        // Phase 1: issue the velocity homing move and wait for the device to acknowledge it.
        {
            // Block scope ends the &mut borrow before the write call below.
            let out = self.connection.get_motor_output_mut();
            out.output_register.set_homing_move(true);
            out.output_register.set_load_velocity_move(true);
            out.jog_velocity = HOMING_VELOCITY_STEPS;
            out.acceleration_limit = HOMING_ACCELERATION_STEPS;

            // EXPERIMENT: try setting deceleration limit to the same as the acceleration limit
            out.deceleration_limit = HOMING_ACCELERATION_STEPS;

            // EXPERIMENT: try setting deceleration limit to the abs of the value defined above
            // out.deceleration_limit = HOMING_DECELERATION_STEPS.abs() as u32;
        }

        self.connection.write_output_assembly().await;
        self.connection
            .poll_motor(|m| m.motor_status.load_velocity_move_ack())
            .await;
        tracing::info!("Velocity move acknowledged — motor is homing");

        // Phase 2: clear the move flags and wait for the homed flag to be set.
        {
            let out = self.connection.get_motor_output_mut();
            out.output_register.set_homing_move(false);
            out.output_register.set_load_velocity_move(false);
        }

        self.connection.write_output_assembly().await;
        self.connection
            .poll_motor(|m| m.motor_status.has_homed())
            .await;
        tracing::info!("Homing complete — has_homed is set");
    }

    async fn disable_motor(&mut self) {
        tracing::info!("Disabling motor...");
        self.connection
            .get_motor_output_mut()
            .output_register
            .set_enable(false);

        self.connection.write_output_assembly().await;
        self.connection
            .poll_motor(|m| !m.motor_status.enabled())
            .await;
        tracing::info!("Motor disabled");
    }

    async fn clear_faults(&mut self) {
        // ---- Phase 1: clear shutdowns if present ----------------

        let assembly = self.connection.read_input_assembly().await;
        if assembly
            .motor_input(self.connection.motor_index)
            .motor_status
            .shutdowns_present()
        {
            tracing::info!("Motor has shutdowns — clearing...");
            {
                let out = self.connection.get_motor_output_mut();
                out.output_register.set_enable(false);
                out.output_register.set_clear_alerts(true);
            }
            self.connection.write_output_assembly().await;
            self.connection
                .poll_motor(|m| !m.motor_status.shutdowns_present())
                .await;

            self.connection
                .get_motor_output_mut()
                .output_register
                .set_clear_alerts(false);
            self.connection.write_output_assembly().await;
            tracing::info!("Shutdowns cleared");
        }

        // ---- Phase 2: clear motor fault if present --------------

        let assembly = self.connection.read_input_assembly().await;
        if assembly
            .motor_input(self.connection.motor_index)
            .motor_status
            .motor_in_fault()
        {
            tracing::info!("Motor is in fault — clearing...");

            // Ensure clear_motor_fault starts low before toggling.
            self.connection
                .get_motor_output_mut()
                .output_register
                .set_clear_motor_fault(false);

            self.connection.write_output_assembly().await;
            self.connection
                .poll_motor(|m| !m.motor_status.clear_motor_fault_ack())
                .await;

            // Toggle high to trigger the clear.
            self.connection
                .get_motor_output_mut()
                .output_register
                .set_clear_motor_fault(true);

            self.connection.write_output_assembly().await;
            self.connection
                .poll_motor(|m| m.motor_status.clear_motor_fault_ack())
                .await;

            // Reset low and wait for acknowledgment to drop.
            self.connection
                .get_motor_output_mut()
                .output_register
                .set_clear_motor_fault(false);

            self.connection.write_output_assembly().await;
            self.connection
                .poll_motor(|m| !m.motor_status.clear_motor_fault_ack())
                .await;

            // Toggle enable (off → on → off) to fully clear motor_in_fault.
            self.connection
                .get_motor_output_mut()
                .output_register
                .set_enable(false);

            self.connection.write_output_assembly().await;
            self.connection
                .poll_motor(|m| !m.motor_status.enabled())
                .await;

            self.connection
                .get_motor_output_mut()
                .output_register
                .set_enable(true);

            self.connection.write_output_assembly().await;
            self.connection
                .poll_motor(|m| !m.motor_status.motor_in_fault())
                .await;

            self.connection
                .get_motor_output_mut()
                .output_register
                .set_enable(false);

            self.connection.write_output_assembly().await;
            self.connection
                .poll_motor(|m| !m.motor_status.enabled())
                .await;

            tracing::info!("Motor fault cleared");
        }
    }
}

// ---- Main ---------------------------------------------------

async fn run_homing(session: &mut HomingSession) {
    session.clear_faults().await;
    session.write_homing_config().await;
    session.enable_motor().await;
    session.wait_for_ready_to_home().await;
    session.home().await;
}

#[tokio::main]
async fn main() {
    tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let mut session = HomingSession::new().await;
    session.register().await;

    tokio::select! {
        _ = run_homing(&mut session) => {
            tracing::info!("Homing sequence completed without interruption");
        },
        _ = tokio::signal::ctrl_c() => {
            tracing::warn!("Ctrl+C received — aborting homing sequence");
        }
    };

    // Always clean up, regardless of how the sequence ended.
    session.disable_motor().await;
    session.unregister().await;
}
