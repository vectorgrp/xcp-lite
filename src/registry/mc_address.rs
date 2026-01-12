// Module address
// McAddress

use serde::{Deserialize, Serialize};

use super::{McIdentifier, Registry};

//-------------------------------------------------------------------------------------------------
// Address Mode Enum

/// Address modes for accessing measurement and calibration data
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum McAddrMode {
    /// Calibration segment relative addressing
    Cal = 0,
    /// Absolute addressing (not implemented for Rust)
    Abs = 1,
    /// Dynamic addressing (event relative addressing, async access)
    Dyn = 2,
    /// Generic A2L address
    A2l = 0xA0,
    /// A2L address with event association
    A2lEvent = 0xA1,
    /// Undefined addressing mode
    Undef = 0xFF,
}

impl Default for McAddrMode {
    fn default() -> Self {
        McAddrMode::Cal
    }
}

impl McAddrMode {
    /// Check if this addressing mode is segment relative
    pub fn is_segment_relative(&self) -> bool {
        matches!(self, McAddrMode::Cal)
    }

    /// Check if this addressing mode is event relative
    pub fn is_event_relative(&self) -> bool {
        matches!(self, McAddrMode::Dyn)
    }

    /// Check if this is an A2L addressing mode
    pub fn is_a2l(&self) -> bool {
        matches!(self, McAddrMode::A2l | McAddrMode::A2lEvent)
    }

    /// Convert to u8 representation
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }
}

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
    addr_offset: i32, // Offset relative to calibration segment (XCP_ADDR_EXT_SEG,calseg_name.is_some) or relative to event base address (XCP_ADDR_EXT_DYN..,event_id.is_some)

    addr_mode: McAddrMode, // Addressing mode

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
            addr_mode: McAddrMode::Cal,
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
    /// Address extension values for the XCP
    pub const XCP_ADDR_EXT_SEG: u8 = 0; // For CAL objects ( index | 0x8000 in high word (CANape does not support addr_ext in memory segments))
    pub const XCP_ADDR_EXT_ABS: u8 = 1; // Not implemented for rust
    pub const XCP_ADDR_EXT_DYN: u8 = 2; // For DAQ objects ( event in addr high word, low word relative to base given to XcpEventExt, async access possible )

    /// Undefined
    pub const XCP_ADDR_EXT_UNDEF: u8 = 0xFF;
    pub const XCP_ADDR_OFFSET_UNDEF: i32 = 0x80000000u32 as i32;

    pub fn new_calseg_rel<T: Into<McIdentifier>>(calseg_name: T, addr_offset: i32) -> Self {
        McAddress {
            calseg_name: Some(calseg_name.into()),
            event_id: None,
            addr_offset,
            addr_mode: McAddrMode::Cal,
            a2l_addr: 0,
            a2l_addr_ext: McAddress::XCP_ADDR_EXT_SEG,
        }
    }

    pub fn new_event_abs(event_id: u16, addr_offset: i32) -> Self {
        McAddress {
            calseg_name: None,
            event_id: Some(event_id),
            addr_offset,
            addr_mode: McAddrMode::Abs,
            a2l_addr: 0,
            a2l_addr_ext: McAddress::XCP_ADDR_EXT_ABS,
        }
    }

    /// Dynamic event relative addressing
    /// # Arguments
    /// * `index` - Index of the dynamic address space (XCP_ADDR_EXT_DYN+index)
    /// * `event_id` - Event id of the measurement signal
    /// * `addr_offset` - Address offset relative to the event base address
    /// # Returns
    /// McAddress instance
    pub fn new_event_dyn(index: u8, event_id: u16, addr_offset: i16) -> Self {
        McAddress {
            calseg_name: None,
            event_id: Some(event_id),
            addr_offset: addr_offset as i32,
            addr_mode: McAddrMode::Dyn,
            a2l_addr: 0,
            a2l_addr_ext: McAddress::XCP_ADDR_EXT_DYN + index,
        }
    }

    // Generic from A2L
    pub fn new_a2l(a2l_addr: u32, a2l_addr_ext: u8) -> Self {
        McAddress {
            calseg_name: None,
            event_id: None,
            addr_offset: McAddress::XCP_ADDR_OFFSET_UNDEF,
            addr_mode: McAddrMode::A2l,
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
            addr_mode: McAddrMode::A2lEvent,
            a2l_addr,
            a2l_addr_ext,
        }
    }

    /// Get address mode
    pub fn get_addr_mode(&self) -> McAddrMode {
        self.addr_mode
    }

    /// Get A2L address extension
    pub fn get_a2l_addr_ext(&self) -> u8 {
        self.a2l_addr_ext
    }

    /// Check address mode is segment relative
    pub fn is_segment_relative(&self) -> bool {
        if self.addr_mode.is_segment_relative() {
            assert!(self.calseg_name.is_some());
            true
        } else {
            false
        }
    }

    /// Check address mode is event relative
    pub fn is_event_relative(&self) -> bool {
        if self.addr_mode.is_event_relative() {
            assert!(self.event_id.is_some());
            true
        } else {
            false
        }
    }

    // Get name of the calibration segment of a calibration object
    pub fn get_calseg_name(&self) -> Option<McIdentifier> {
        self.calseg_name
    }

    // Get event id of the event associated with a measurement signal
    pub fn get_event_id(&self) -> Option<u16> {
        self.event_id
    }
    pub fn get_event_id_unchecked(&self) -> u16 {
        self.get_event_id().unwrap_or(
            0xFFFF, // Invalid event id, used in sorting by event id
        )
    }

    /// Get relative address offset to event or calibration segment
    /// # Panics
    /// If the address is not segment or event relative
    pub fn get_addr_offset(&self) -> i32 {
        match self.addr_mode {
            McAddrMode::Cal | McAddrMode::Dyn => self.addr_offset,
            McAddrMode::A2l | McAddrMode::A2lEvent => panic!("A2L address does not have an offset"),
            McAddrMode::Abs | McAddrMode::Undef => panic!("Address mode not supported"),
        }
    }

    /// Add an offset to an address
    pub fn add_addr_offset(&mut self, offset: i32) {
        match self.addr_mode {
            McAddrMode::Cal | McAddrMode::Dyn => {
                self.addr_offset += offset;
            }
            McAddrMode::A2l | McAddrMode::A2lEvent => {
                self.a2l_addr = (self.a2l_addr as i64 + offset as i64) as u32;
            }
            McAddrMode::Abs => {
                self.addr_offset += offset;
            }
            McAddrMode::Undef => panic!("Address mode Undef"),
        }
    }

    fn get_abs_ext_addr(offset: u32) -> (u8, u32) {
        let a2l_ext = McAddress::XCP_ADDR_EXT_ABS;
        #[allow(clippy::cast_sign_loss)]
        let a2l_addr: u32 = offset;
        (a2l_ext, a2l_addr)
    }

    fn get_dyn_ext_addr(addr_ext: u8, event_id: u16, offset: i16) -> (u8, u32) {
        // @@@@ TODO: Improve range check for DYN addr_ext ????
        assert!(
            addr_ext >= McAddress::XCP_ADDR_EXT_DYN && addr_ext < McAddress::XCP_ADDR_EXT_DYN + 16,
            "Invalid addr_ext for DYN addressing"
        );

        #[allow(clippy::cast_sign_loss)]
        let a2l_addr: u32 = ((event_id as u32) << 16) | (offset as u16 as u32);
        (addr_ext, a2l_addr)
    }

    // Get A2L addr (ext,addr) of a CalSeg
    pub fn get_calseg_ext_addr_base(calseg_index: u16) -> (u8, u32) {
        // McAddress format for calibration segment field is index | 0x8000 in high word, addr_ext is 0 (CANape does not support addr_ext in memory segments)
        let addr_ext = McAddress::XCP_ADDR_EXT_SEG;
        let addr = ((calseg_index as u32) | 0x8000) << 16;
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
        match self.addr_mode {
            // Event relative addressing with async access
            McAddrMode::Dyn => McAddress::get_dyn_ext_addr(self.a2l_addr_ext, self.event_id.unwrap(), self.addr_offset.try_into().expect("offset too large")),
            // Absolute addressing with default event
            McAddrMode::Abs => McAddress::get_abs_ext_addr(self.addr_offset.try_into().expect("get_a2l_addr: addr too large")),
            // Explicit segment relative addressing
            McAddrMode::Cal => {
                let name = self.calseg_name.as_ref().expect("get_a2l_addr: Calibration segment name not set");
                let index = registry
                    .cal_seg_list
                    .get_cal_seg_index(name)
                    .unwrap_or_else(|| panic!("get_a2l_addr: Calibration segment {} not found", name));
                McAddress::get_calseg_ext_addr(index, self.addr_offset.try_into().expect("get_a2l_addroffset too large"))
            }
            // Explicit A2L address
            McAddrMode::A2l | McAddrMode::A2lEvent => (self.a2l_addr_ext, self.a2l_addr),
            // Undefined address mode
            McAddrMode::Undef => panic!("get_a2l_addr: Undefined address mode"),
        }
    }

    // Get raw A2L addr (ext,addr) stored in the McAddress
    // This is used when the address is imported from a third party A2L file
    // No conversion is done
    // # Panics
    // If the address mode is not A2L
    pub fn get_raw_a2l_addr(&self) -> (u8, u32) {
        assert!(self.addr_mode.is_a2l(), "Raw A2L address is only available for A2L addressing modes");
        (self.a2l_addr_ext, self.a2l_addr)
    }
    // Set the A2L address and address extension
    // Internally used when updating an A2L file
    pub fn set_raw_a2l_addr(&mut self, a2l_addr_ext: u8, a2l_addr: u32) {
        assert!(self.addr_mode.is_a2l());
        self.a2l_addr = a2l_addr;
        self.a2l_addr_ext = a2l_addr_ext;
    }
}

impl std::fmt::Display for McAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)?;
        Ok(())
    }
}

// Implement cmp and ord for sorting by event_id, addr_ext, addr, name
impl PartialOrd for McAddress {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for McAddress {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let mode1 = self.addr_mode;
        let mode2 = other.addr_mode;
        if mode1 != mode2 {
            mode1.cmp(&mode2)
        } else if self.addr_mode.is_a2l() {
            if self.a2l_addr_ext != other.a2l_addr_ext {
                self.a2l_addr_ext.cmp(&other.a2l_addr_ext)
            } else if self.a2l_addr != other.a2l_addr {
                self.a2l_addr.cmp(&other.a2l_addr)
            } else {
                std::cmp::Ordering::Equal
            }
        } else {
            self.addr_offset.cmp(&other.addr_offset)
        }
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
        reg.cal_seg_list.add_a2l_cal_seg("calseg", 0, 0, 0x80000000, 0x1000).unwrap();

        let addr = McAddress::new_calseg_rel("calseg", 11);
        assert_eq!(addr.get_calseg_name(), Some(McIdentifier::new("calseg")));
        assert_eq!(addr.get_event_id(), None);
        assert_eq!(addr.get_addr_offset(), 11);
        let a = addr.get_a2l_addr(&reg);
        assert!(a.0 == McAddress::XCP_ADDR_EXT_SEG);
        assert_eq!(a.1, 0x80000000 + 11);

        {
            let addr = McAddress::new_event_dyn(0, 2, -1);
            assert_eq!(addr.get_calseg_name(), None);
            assert_eq!(addr.get_event_id(), Some(2));
            assert_eq!(addr.get_addr_offset(), -1);
            let a = addr.get_a2l_addr(&reg);
            assert!(a.0 == McAddress::XCP_ADDR_EXT_DYN);
            assert_eq!(a.1, 0x0002FFFF);

            let addr = McAddress::new_event_dyn(0, 2, 0x7FFF);
            assert_eq!(addr.get_calseg_name(), None);
            assert_eq!(addr.get_event_id(), Some(2));
            assert_eq!(addr.get_addr_offset(), 0x7FFF);
            let a = addr.get_a2l_addr(&reg);
            assert!(a.0 == McAddress::XCP_ADDR_EXT_DYN);
            assert_eq!(a.1, 0x00027FFF);
        }
    }
}
