// Module mc_calseg
// Types:
//  McTypeDef, McTypeDefField

use std::borrow::Cow;

use serde::Deserialize;
use serde::Serialize;

use super::McAddress;
use super::McIdentifier;
use super::Registry;
use super::RegistryError;

//-------------------------------------------------------------------------------------------------
// Calibration segments

// A range of continuous memory which contains only calibration parameters
// Calibration parameters belong to a calibration segment when their address is in this range
// Calibration parameters will never be changed by the application
// The calibration tool is then able to modify contents of the calibration segment in a thread safe way
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize)]
pub struct McCalibrationSegment {
    pub name: McIdentifier, // Unique name of the calibration segment
    pub index: u16,         // Unique index of the calibration segment, used for relative addressing
    pub addr: u32,          // A2L address
    pub addr_ext: u8,       // A2L address extension
    pub mem_addr: u64,      // Memory address
    pub size: u32,          // Size in bytes
    pub number: Option<u8>, // Number used for XCP protocol and A2L MEMORY_SEGMENT definition
}

impl McCalibrationSegment {
    pub fn new<T: Into<McIdentifier>>(name: T, index: u16, addr: u32, addr_ext: u8, size: u32, number: Option<u8>) -> McCalibrationSegment {
        let name: McIdentifier = name.into();
        McCalibrationSegment {
            name,
            index,
            addr,
            addr_ext,
            mem_addr: 0,
            size,
            number,
        }
    }

    /// Get the calibration segment name
    pub fn get_name(&self) -> &'static str {
        self.name.as_str()
    }

    /// Get the calibration segment index
    pub fn get_index(&self) -> u16 {
        self.index
    }

    /// Get the calibration segment number
    pub fn get_number(&self) -> Option<u8> {
        self.number
    }

    // Set the calibration segment index
    // For internal use only
    pub fn set_index(&mut self, index: u16) {
        self.index = index;
    }

    // Set the calibration segment number
    // For internal use only
    pub fn set_number(&mut self, number: Option<u8>) {
        self.number = number;
    }

    /// Set calibration segment memory address
    /// For internal use only
    pub fn set_mem_addr(&mut self, mem_addr: u64) {
        self.mem_addr = mem_addr;
    }

    /// Get the full indexed name of the calibration segment
    /// The calibration segment name may not be unique, segments with the same name may be created by multiple thread instances of a task, this is indicated by index > 0
    /// The name is prefixed with the application name if prefix_names is set
    pub fn get_prefixed_name(&self, registry: &Registry) -> Cow<'static, str> {
        if registry.get_prefix_names_mode() {
            Cow::Owned(format!("{}.{}", registry.application.get_name(), self.name))
        } else {
            Cow::Borrowed(self.name.as_str())
        }
    }
}

//----------------------------------------------------------------------------------------------
// McCalibrationSegmentList

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct McCalibrationSegmentList(pub Vec<McCalibrationSegment>);

impl McCalibrationSegmentList {
    pub fn new() -> Self {
        McCalibrationSegmentList(Vec::with_capacity(8))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn push(&mut self, object: McCalibrationSegment) {
        self.0.push(object);
    }

    pub fn get(&self, index: usize) -> Option<&McCalibrationSegment> {
        self.0.get(index)
    }

    pub fn sort_by_name(&mut self) {
        self.0.sort_by(|a, b| a.name.cmp(&b.name));
    }

    /// Add a calibration segment with segment relative addressing mode
    pub fn add_cal_seg<T: Into<McIdentifier>>(&mut self, name: T, number: Option<u8>, size: u32) -> Result<&McCalibrationSegment, RegistryError> {
        let index = u16::try_from(self.len()).map_err(|_| RegistryError::IndexOverflow)?;
        let (addr_ext, addr) = McAddress::get_calseg_ext_addr_base(index);
        self.add_a2l_cal_seg(name, index, number, addr_ext, addr, size)
    }

    /// Add a calibration segment with absolute addressing mode
    pub fn add_cal_seg_by_addr<T: Into<McIdentifier>>(&mut self, name: T, number: Option<u8>, addr_ext: u8, addr: u32, size: u32) -> Result<&McCalibrationSegment, RegistryError> {
        let index = u16::try_from(self.len()).map_err(|_| RegistryError::IndexOverflow)?;
        self.add_a2l_cal_seg(name, index, number, addr_ext, addr, size)
    }

    /// Add a calibration segment by name, index, address extension and address
    pub fn add_a2l_cal_seg<T: Into<McIdentifier>>(
        &mut self,
        name: T,
        index: u16,
        number: Option<u8>,
        addr_ext: u8,
        addr: u32,
        size: u32,
    ) -> Result<&McCalibrationSegment, RegistryError> {
        let name: McIdentifier = name.into();

        // Check if name already exists and panic
        for s in &self.0 {
            if s.name == name {
                log::warn!("Duplicate calibration segment {}!", name);
                return Err(RegistryError::Duplicate(name.to_string()));
            }
        }

        // Check if number already exists
        if let Some(number) = number {
            if self.find_cal_seg_by_number(number).is_some() {
                let error_msg = format!("Duplicate calibration segment number {}!", number);
                log::error!("{}", error_msg);
                return Err(RegistryError::Duplicate(error_msg));
            }
        }

        log::debug!("Registry add_cal_seg: {} {} {}:0x{:08X}-{} ", name, index, addr_ext, addr, size);
        let seg = McCalibrationSegment::new(name, index, addr, addr_ext, size, number);
        self.push(seg);
        Ok(self.0.last().unwrap())
    }

    /// Find a calibration segment by name
    pub fn find_cal_seg(&mut self, name: &str) -> Option<&mut McCalibrationSegment> {
        self.into_iter().find(|i| i.name == name)
    }

    /// Find a calibration segment name by the a2l address of a calibration parameter
    /// Returns the name of the calibration segment
    pub fn find_cal_seg_by_a2l_address(&self, addr: u32) -> Option<McIdentifier> {
        self.into_iter().find(|i| i.addr <= addr && addr < i.addr + i.size).map(|s| s.name)
    }

    /// Find a calibration segment name by the memory address of a calibration parameter
    /// Returns the name of the calibration segment
    pub fn find_cal_seg_by_mem_address(&self, mem_addr: u64) -> Option<McIdentifier> {
        self.into_iter().find(|i| i.mem_addr <= mem_addr && mem_addr < i.mem_addr + i.size as u64).map(|s| s.name)
    }

    /// Find a calibration segment name by index
    /// Returns the name of the calibration segment
    pub fn find_cal_seg_by_index(&self, index: u16) -> Option<McIdentifier> {
        self.into_iter().find(|i| i.index == index).map(|s| s.name)
    }

    /// Find a calibration segment name by number
    /// Returns the name of the calibration segment
    pub fn find_cal_seg_by_number(&self, number: u8) -> Option<McIdentifier> {
        self.into_iter().find(|i| i.number == Some(number)).map(|s| s.name)
    }

    /// Get calibration segment index by name
    /// Index ist used to build addressing information in the XCP protocol
    pub fn get_cal_seg_index(&self, name: &str) -> Option<u16> {
        for s in self {
            if s.name == name {
                return Some(s.index);
            }
        }
        None
    }
}

//-------------------------------------------------------------------------------------------------
// McCalibrationSegmentListIterator

/// Iterator for EventList
pub struct McCalibrationSegmentListIterator<'a> {
    index: usize,
    list: &'a McCalibrationSegmentList,
}

impl<'a> McCalibrationSegmentListIterator<'_> {
    pub fn new(list: &'a McCalibrationSegmentList) -> McCalibrationSegmentListIterator<'a> {
        McCalibrationSegmentListIterator { index: 0, list }
    }
}

impl<'a> Iterator for McCalibrationSegmentListIterator<'a> {
    type Item = &'a McCalibrationSegment;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.index;
        if index < self.list.0.len() {
            self.index += 1;
            Some(&self.list.0[index])
        } else {
            None
        }
    }
}

impl<'a> IntoIterator for &'a McCalibrationSegmentList {
    type Item = &'a McCalibrationSegment;
    type IntoIter = McCalibrationSegmentListIterator<'a>;

    fn into_iter(self) -> McCalibrationSegmentListIterator<'a> {
        McCalibrationSegmentListIterator::new(self)
    }
}

//-------------------------------------------------------------------------------------------------
// McCalibrationSegmentListIteratorMut (Mutable Iterator)

/// Mutable iterator for CalibrationSegmentList
pub struct McCalibrationSegmentListIteratorMut<'a> {
    iter: std::slice::IterMut<'a, McCalibrationSegment>,
}

impl<'a> McCalibrationSegmentListIteratorMut<'a> {
    pub fn new(list: &'a mut McCalibrationSegmentList) -> McCalibrationSegmentListIteratorMut<'a> {
        McCalibrationSegmentListIteratorMut { iter: list.0.iter_mut() }
    }
}

impl<'a> Iterator for McCalibrationSegmentListIteratorMut<'a> {
    type Item = &'a mut McCalibrationSegment;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl<'a> IntoIterator for &'a mut McCalibrationSegmentList {
    type Item = &'a mut McCalibrationSegment;
    type IntoIter = McCalibrationSegmentListIteratorMut<'a>;

    fn into_iter(self) -> McCalibrationSegmentListIteratorMut<'a> {
        McCalibrationSegmentListIteratorMut::new(self)
    }
}
