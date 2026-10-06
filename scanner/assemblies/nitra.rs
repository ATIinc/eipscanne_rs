//! The assemblies of a Nitra EtherNet/IP pneumatic valve manifold:
//! https://cdn.automationdirect.com/static/manuals/nitrainserts/nitra_ump_ethernetip.pdf#page=6

use binrw::{BinRead, BinWrite};

use bilge::prelude::{BuilderBits, DebugBits, DefaultBits, FromBits, bitsize};

/// Assembly instance that drives the solenoid valves (outputs)
pub const VALVES_ASSEMBLY_INSTANCE: u8 = 100;
/// Assembly instance that reports the manifold status (inputs)
pub const STATUS_ASSEMBLY_INSTANCE: u8 = 101;
/// Number of valves on the manifold
pub const VALVE_COUNT: usize = 16;

/// One bit per solenoid valve; `true` energizes the valve
#[bitsize(16)]
#[derive(
    FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone, BuilderBits, DefaultBits,
)]
#[br(map = u16::into)]
#[bw(map = |&x| u16::from(x))]
pub struct SolenoidValves {
    pub valve0: bool,
    pub valve1: bool,
    pub valve2: bool,
    pub valve3: bool,
    pub valve4: bool,
    pub valve5: bool,
    pub valve6: bool,
    pub valve7: bool,
    pub valve8: bool,
    pub valve9: bool,
    pub valve10: bool,
    pub valve11: bool,
    pub valve12: bool,
    pub valve13: bool,
    pub valve14: bool,
    pub valve15: bool,
}

// ======= Start of SolenoidValves impl ========

impl SolenoidValves {
    /// Sets valve `index` (0-15) to `energized`
    pub fn set_valve(&mut self, index: usize, energized: bool) {
        match index {
            0 => self.set_valve0(energized),
            1 => self.set_valve1(energized),
            2 => self.set_valve2(energized),
            3 => self.set_valve3(energized),
            4 => self.set_valve4(energized),
            5 => self.set_valve5(energized),
            6 => self.set_valve6(energized),
            7 => self.set_valve7(energized),
            8 => self.set_valve8(energized),
            9 => self.set_valve9(energized),
            10 => self.set_valve10(energized),
            11 => self.set_valve11(energized),
            12 => self.set_valve12(energized),
            13 => self.set_valve13(energized),
            14 => self.set_valve14(energized),
            15 => self.set_valve15(energized),
            _ => panic!("valve index must be 0-{}, got {index}", VALVE_COUNT - 1),
        }
    }
}

// ^^^^^^^^ End of SolenoidValves impl ^^^^^^^^

/// The manifold's status byte; the manual documents the meaning of each bit
#[bitsize(8)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
#[br(map = u8::into)]
#[bw(map = |&x| u8::from(x))]
pub struct StatusByte {
    pub bit0: bool,
    pub bit1: bool,
    pub bit2: bool,
    pub bit3: bool,
    pub bit4: bool,
    pub bit5: bool,
    pub bit6: bool,
    pub bit7: bool,
}
