use binrw::{
    binrw,
    BinRead,
    BinWrite, // #[binrw] attribute
              // BinRead,  // trait for reading
              // BinWrite, // trait for writing
};

use bilge::prelude::{bitsize, Bitsized, DebugBits, FromBits, Number};

#[bitsize(16)]
#[derive(FromBits, PartialEq, DebugBits, BinRead, BinWrite, Copy, Clone)]
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
    pub fn default() -> Self {
        SolenoidValves::new(
            false, false, false, false, false, false, false, false, false, false, false, false,
            false, false, false, false,
        )
    }

    pub fn set_valve_index(&mut self, index: usize, value: bool) {
        match index {
            0 => self.set_valve0(value),
            1 => self.set_valve1(value),
            2 => self.set_valve2(value),
            3 => self.set_valve3(value),
            4 => self.set_valve4(value),
            5 => self.set_valve5(value),
            6 => self.set_valve6(value),
            7 => self.set_valve7(value),
            8 => self.set_valve8(value),
            9 => self.set_valve9(value),
            10 => self.set_valve10(value),
            11 => self.set_valve11(value),
            12 => self.set_valve12(value),
            13 => self.set_valve13(value),
            14 => self.set_valve14(value),
            15 => self.set_valve15(value),
            _ => panic!("Index out of range"),
        }
    }
}

// ^^^^^^^^ End of SolenoidValves impl ^^^^^^^^

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
