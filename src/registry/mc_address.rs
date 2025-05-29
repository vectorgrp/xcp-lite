// Module address
// McAddress

use serde::{Deserialize, Serialize};

use super::{McIdentifier, Registry};

//-------------------------------------------------------------------------------------------------
// McAddress
// Information needed to access data instances

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McAddress {
    #[serde(skip_serializing_if = "Option::is_none")]
    calseg_name: Option<McIdentifier>, // Name of the calibration segment of a calibration object

    #[serde(skip_serializing_if = "Option::is_none")]
    event_id: Option<u16>, // Event id of a measurement signal

    #[serde(default = "default_addr_offset")]
    #[serde(skip_serializing_if = "skip_addr_offset")]
    addr_offset: i32, // Offset relative to calibration segment (XCP_ADDR_EXT_SEG,calseg_name.is_some) or relative to event base address (XCP_ADDR_EXT_REL,event_id.is_some)

    addr_mode: u8, // Mode ADDR_MODE_CAL, ADDR_MODE_DYN, ADDR_MODE_REL, ADDR_MODE_A2L, ADDR_MODE_A2L_VECTOR

    #[serde(default = "default_a2l_addr")]
    #[serde(skip_serializing_if = "skip_a2l_addr")]
    a2l_addr: u32, // XCP address, used if data description is generated from a third party A2L file

    #[serde(default = "default_a2l_addr_ext")]
    #[serde(skip_serializing_if = "skip_a2l_addr_ext")]
    a2l_addr_ext: u8, // XCP address extensionAddress,, used if data description is generated from a third party A2L file, otherwise XCP_ADDR_EXT_UNDEF
}

impl Default for McAddress {
    fn default() -> Self {
        McAddress {
            calseg_name: None,
            event_id: None,
            addr_offset: McAddress::XCP_ADDR_OFFSET_UNDEF,
            addr_mode: McAddress::ADDR_MODE_UNDEF,
            a2l_addr: 0,
            a2l_addr_ext: 0,
        }
    }
}

// Defaults for deserializer
fn default_a2l_addr_ext() -> u8 {
    0
}
fn default_a2l_addr() -> u32 {
    0
}
fn default_addr_offset() -> i32 {
    McAddress::XCP_ADDR_OFFSET_UNDEF
}

// Skip on defaults
fn skip_a2l_addr_ext(value: &u8) -> bool {
    *value == 0
}
fn skip_a2l_addr(value: &u32) -> bool {
    *value == 0
}
fn skip_addr_offset(value: &i32) -> bool {
    *value == McAddress::XCP_ADDR_OFFSET_UNDEF
}

impl McAddress {
    /// Addressing modes
    pub const ADDR_MODE_CAL: u8 = 0;
    pub const ADDR_MODE_ABS: u8 = 1; // Not implemented for rust
    pub const ADDR_MODE_DYN: u8 = 2; // Note: SHM mode does not support dynamic addressing, calibration access is done via shared memory only
    pub const ADDR_MODE_REL: u8 = 3;
    pub const ADDR_MODE_A2L: u8 = 0xA0;
    pub const ADDR_MODE_A2L_EVENT: u8 = 0xA1;
    pub const ADDR_MODE_UNDEF: u8 = 0xFF;

    /// Address extension values for the XCP
    pub const XCP_ADDR_EXT_SEG: u8 = 0; // For CAL objects ( index | 0x8000 in high word (CANape does not support addr_ext in memory segments))
    pub const XCP_ADDR_EXT_ABS: u8 = 1; // Not implemented for rust
    pub const XCP_ADDR_EXT_DYN: u8 = 2; // For DAQ objects ( event in addr high word, low word relative to base given to XcpEventExt, async access possible )
    pub const XCP_ADDR_EXT_REL: u8 = 3; // For DAQ objects ( event in addr high word, low word relative to base given to XcpEventExt, no async access )

    /// Undefined
    pub const XCP_ADDR_EXT_UNDEF: u8 = 0xFF;
    pub const XCP_ADDR_OFFSET_UNDEF: i32 = 0x80000000u32 as i32;

    /// Addr of the EPK used
    pub const XCP_EPK_ADDR: u32 = 0x80000000;

    pub fn new_calseg_rel<T: Into<McIdentifier>>(calseg_name: T, addr_offset: i32) -> Self {
        McAddress {
            calseg_name: Some(calseg_name.into()),
            event_id: None,
            addr_offset,
            addr_mode: McAddress::ADDR_MODE_CAL,
            a2l_addr: 0,
            a2l_addr_ext: 0,
        }
    }

    pub fn new_event_rel(event_id: u16, addr_offset: i32) -> Self {
        McAddress {
            calseg_name: None,
            event_id: Some(event_id),
            addr_offset,
            addr_mode: McAddress::ADDR_MODE_REL,
            a2l_addr: 0,
            a2l_addr_ext: 0,
        }
    }

    pub fn new_event_dyn(event_id: u16, addr_offset: i16) -> Self {
        McAddress {
            calseg_name: None,
            event_id: Some(event_id),
            addr_offset: addr_offset as i32,
            addr_mode: McAddress::ADDR_MODE_DYN,
            a2l_addr: 0,
            a2l_addr_ext: 0,
        }
    }

    // Generic from A2L
    pub fn new_a2l(a2l_addr: u32, a2l_addr_ext: u8) -> Self {
        McAddress {
            calseg_name: None,
            event_id: None,
            addr_offset: McAddress::XCP_ADDR_OFFSET_UNDEF,
            addr_mode: McAddress::ADDR_MODE_A2L,
            a2l_addr,
            a2l_addr_ext,
        }
    }

    // Generic from A2L with IF_DATA XCP event
    pub fn new_a2l_with_event(event_id: u16, a2l_addr: u32, a2l_addr_ext: u8) -> Self {
        McAddress {
            calseg_name: None,
            event_id: Some(event_id),
            addr_offset: 0,
            addr_mode: McAddress::ADDR_MODE_A2L_EVENT,
            a2l_addr,
            a2l_addr_ext,
        }
    }

    /// Check address mode is segment relative
    pub fn is_segment_relative(&self) -> bool {
        if self.addr_mode == McAddress::ADDR_MODE_CAL {
            assert!(self.calseg_name.is_some());
            true
        } else {
            false
        }
    }

    /// Check address mode is event relative
    pub fn is_event_relative(&self) -> bool {
        if self.addr_mode == McAddress::ADDR_MODE_REL || self.addr_mode == McAddress::ADDR_MODE_DYN {
            assert!(self.event_id.is_some());
            true
        } else {
            false
        }
    }

    // Get name of the calibration segment of a calibration object
    pub fn calseg_name(&self) -> Option<McIdentifier> {
        self.calseg_name
    }

    // Get event id of the event associated with a measurement signal
    pub fn event_id(&self) -> Option<u16> {
        self.event_id
    }
    pub fn get_event_id_unchecked(&self) -> u16 {
        self.event_id().unwrap_or(
            0xFFFF, // Invalid event id, used in sorting by event id
        )
    }

    /// Get relative address offset to event or calibration segment
    /// # Panics
    /// If the address is not segment or event relative
    pub fn get_addr_offset(&self) -> i32 {
        match self.addr_mode {
            McAddress::ADDR_MODE_REL | McAddress::ADDR_MODE_CAL | McAddress::ADDR_MODE_DYN => self.addr_offset,
            McAddress::ADDR_MODE_A2L | McAddress::ADDR_MODE_A2L_EVENT => panic!("A2L address does not have an offset"),
            _ => panic!("Invalid address mode"),
        }
    }

    /// Add an offset to an address
    /// # Panics
    /// If the address is not segment or event relative
    pub fn add_addr_offset(&mut self, offset: i32) {
        match self.addr_mode {
            McAddress::ADDR_MODE_REL | McAddress::ADDR_MODE_CAL | McAddress::ADDR_MODE_DYN => {
                self.addr_offset += offset;
            }
            McAddress::ADDR_MODE_A2L | McAddress::ADDR_MODE_A2L_EVENT => panic!("A2L address does not have an offset"),
            _ => panic!("Invalid address mode"),
        }
    }

    fn get_dyn_ext_addr(event_id: u16, offset: i16) -> (u8, u32) {
        let a2l_ext = McAddress::XCP_ADDR_EXT_DYN;
        #[allow(clippy::cast_sign_loss)]
        let a2l_addr: u32 = ((event_id as u32) << 16) | (offset as u16 as u32);
        (a2l_ext, a2l_addr)
    }

    fn get_rel_ext_addr(offset: i32) -> (u8, u32) {
        let a2l_ext = McAddress::XCP_ADDR_EXT_REL;
        #[allow(clippy::cast_sign_loss)]
        let a2l_addr: u32 = offset as u32;
        (a2l_ext, a2l_addr)
    }

    // Get A2L addr (ext,addr) of a CalSeg
    pub fn get_calseg_ext_addr_base(calseg_index: u16) -> (u8, u32) {
        // McAddress format for calibration segment field is index | 0x8000 in high word, addr_ext is 0 (CANape does not support addr_ext in memory segments)
        let addr_ext = McAddress::XCP_ADDR_EXT_SEG;
        let addr = (((calseg_index as u32) + 1) | 0x8000) << 16;
        (addr_ext, addr)
    }

    // Get A2L addr (ext,addr) for a calibration value field at offset in a CalSeg
    // The address is relative to the base addr of the calibration segment
    pub fn get_calseg_ext_addr(calseg_index: u16, offset: u16) -> (u8, u32) {
        {
            let (addr_ext, mut addr) = McAddress::get_calseg_ext_addr_base(calseg_index);
            addr += offset as u32;
            (addr_ext, addr)
        }
    }

    /// Get address extension and address for A2L generation and the XCP protocol
    pub fn get_a2l_addr(&self, registry: &Registry) -> (u8, u32) {
        // Event relative addressing
        if self.addr_mode == McAddress::ADDR_MODE_REL {
            McAddress::get_rel_ext_addr(self.addr_offset)
        }
        // Event relative addressing with async access
        else if self.addr_mode == McAddress::ADDR_MODE_DYN {
            McAddress::get_dyn_ext_addr(self.event_id.unwrap(), self.addr_offset.try_into().expect("offset too large"))
        }
        // Explicit segment relative addressing
        else if self.addr_mode == McAddress::ADDR_MODE_CAL {
            let index = registry
                .cal_seg_list
                .get_cal_seg_index(self.calseg_name.as_ref().unwrap())
                .expect("Relative addressing needs a calibration segment");
            McAddress::get_calseg_ext_addr(index, self.addr_offset.try_into().expect("offset too large"))
        }
        // Explicit A2L address
        else if self.addr_mode == McAddress::ADDR_MODE_A2L || self.addr_mode == McAddress::ADDR_MODE_A2L_EVENT {
            (self.a2l_addr_ext, self.a2l_addr)
        } else {
            panic!("Invalid address mode")
        }
    }
}

impl std::fmt::Display for McAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)?;
        Ok(())
    }
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
// Test module

#[cfg(test)]
mod mc_address_tests {

    use crate::xcp::xcp_test::test_setup;

    use super::*;

    #[test]
    fn test_mc_address() {
        let _ = test_setup();

        let mut reg = Registry::new();
        reg.cal_seg_list.add_a2l_cal_seg("calseg", 0, 0, 0x80010000, 0x1000).unwrap();

        let addr = McAddress::new_calseg_rel("calseg", 11);
        assert_eq!(addr.calseg_name(), Some(McIdentifier::new("calseg")));
        assert_eq!(addr.event_id(), None);
        assert_eq!(addr.get_addr_offset(), 11);
        let a = addr.get_a2l_addr(&reg);
        assert!(a.0 == McAddress::XCP_ADDR_EXT_SEG);
        assert_eq!(a.1, 0x8001000B);

        let addr = McAddress::new_event_rel(1, -1);
        assert_eq!(addr.calseg_name(), None);
        assert_eq!(addr.event_id(), Some(1));
        assert_eq!(addr.get_addr_offset(), -1);
        let a = addr.get_a2l_addr(&reg);
        assert!(a.0 == McAddress::XCP_ADDR_EXT_REL);
        assert_eq!(a.1, 0xFFFFFFFF);

        let addr = McAddress::new_event_rel(1, 0x7FFF_FFFF);
        assert_eq!(addr.calseg_name(), None);
        assert_eq!(addr.event_id(), Some(1));
        assert_eq!(addr.get_addr_offset(), 0x7FFF_FFFF);
        let a = addr.get_a2l_addr(&reg);
        assert!(a.0 == McAddress::XCP_ADDR_EXT_REL);
        assert_eq!(a.1, 0x7FFFFFFF);

        {
            let addr = McAddress::new_event_dyn(2, -1);
            assert_eq!(addr.calseg_name(), None);
            assert_eq!(addr.event_id(), Some(2));
            assert_eq!(addr.get_addr_offset(), -1);
            let a = addr.get_a2l_addr(&reg);
            assert!(a.0 == McAddress::XCP_ADDR_EXT_DYN);
            assert_eq!(a.1, 0x0002FFFF);

            let addr = McAddress::new_event_dyn(2, 0x7FFF);
            assert_eq!(addr.calseg_name(), None);
            assert_eq!(addr.event_id(), Some(2));
            assert_eq!(addr.get_addr_offset(), 0x7FFF);
            let a = addr.get_a2l_addr(&reg);
            assert!(a.0 == McAddress::XCP_ADDR_EXT_DYN);
            assert_eq!(a.1, 0x00027FFF);
        }
    }
}
