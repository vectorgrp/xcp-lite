// Module mc_registry
// Types:
//  Registry

use log::info;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::net::Ipv4Addr;

//use super::McAddress;
use super::McCalibrationSegmentList;
use super::McDimType;
use super::McEventList;
use super::McIdentifier;
use super::McInstanceList;
use super::McObjectType;
use super::McSupportData;
use super::McText;
use super::McTypeDef;
use super::McTypeDefList;
use super::McXcpTransportLayer;
use super::RegistryError;
use super::flatten_registry;

//-------------------------------------------------------------------------------------------------
// McApplicationVersion
// Software version string identifier or EPK with address

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize)]
pub struct McApplicationVersion {
    pub epk: McText,
    pub epk_addr: u32,
}

// impl McApplicationVersion {
//     fn new() -> McApplicationVersion {
//         McApplicationVersion::default()
//     }
// }

impl Default for McApplicationVersion {
    fn default() -> Self {
        McApplicationVersion { epk: "".into(), epk_addr: 0 }
    }
}

//-------------------------------------------------------------------------------------------------
// Application

/// Infos on the application
#[derive(Debug, Default, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize)]
pub struct McApplication {
    pub app_id: u8,                    // Unique identifier for the application
    pub name: McIdentifier,            // Name of the application, used as A2L filename and module name
    pub description: McText,           // Optional description of the application
    pub version: McApplicationVersion, // Version or EPK string with address
}

impl McApplication {
    pub fn new() -> McApplication {
        McApplication {
            app_id: 0,
            name: "".into(),
            description: "".into(),
            version: McApplicationVersion::default(),
        }
    }

    /// Check if a version string or EPK and address is available for the application
    pub fn has_epk(&self) -> bool {
        !self.version.epk.is_empty()
    }

    /// Set application name
    pub fn set_info<A: Into<McIdentifier>, B: Into<McText>>(&mut self, name: A, description: B, id: u8) {
        let name: McIdentifier = name.into();
        let description: McText = description.into();
        log::info!("Registry set application info, app_name='{}', app_id={}, description='{}'", name, id, description);

        // Set name, id and description
        self.app_id = id;
        self.name = name;
        self.description = description;
    }

    /// Get application name
    pub fn get_name(&self) -> &'static str {
        if !self.name.is_empty() { self.name.as_str() } else { "application" }
    }

    /// Set application version
    pub fn set_version<T: Into<McText>>(&mut self, epk: T, epk_addr: u32) {
        let epk: McText = epk.into();
        log::debug!("Registry set epk: {} 0x{:08X}", epk, epk_addr);
        self.version.epk = epk;
        self.version.epk_addr = epk_addr;
    }

    /// Get application version
    pub fn get_version(&self) -> &str {
        self.version.epk.as_str()
    }
}

//-------------------------------------------------------------------------------------------------
// Registry

/// Measurement and calibration object database
#[derive(Debug, Serialize, Deserialize)]
pub struct Registry {
    // Flatten typedefs to measurement and calibration objects when writing A2L
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    flatten_typedefs: bool,

    // Prefix name wit application name when writing A2L
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    prefix_names: bool,

    // Application name and software version
    pub application: McApplication,

    // XCP transport layer parameters
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    pub xcp_tl_params: Option<McXcpTransportLayer>,

    // All eventss
    pub event_list: McEventList,

    // All calibration segments, sorted list
    pub cal_seg_list: McCalibrationSegmentList,

    // All typedefs, sorted list
    pub typedef_list: McTypeDefList,

    // All measurement and calibration objects, sorted list
    pub instance_list: McInstanceList,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    /// Create a measurement and calibration registry
    pub fn new() -> Registry {
        Registry {
            flatten_typedefs: false,
            prefix_names: false,
            application: McApplication::new(),
            xcp_tl_params: None,
            event_list: McEventList::new(),
            cal_seg_list: McCalibrationSegmentList::new(),
            typedef_list: McTypeDefList::new(),
            instance_list: McInstanceList::new(),
        }
    }

    //---------------------------------------------------------------------------------------------------------
    // XCP parameters (ID_DATA XCP)

    /// Set XCP transport layer parameters for Ethernet and enable XCP IF_DATA in A2L
    /// @param protocol_name: Name of the protocol (e.g. "UDP", "TCP")
    /// @param addr: IP address of the Ethernet interface
    /// @param port: Port number of the Ethernet interface
    pub fn set_xcp_eth_params(&mut self, protocol_name: &'static str, addr: Ipv4Addr, port: u16) {
        log::debug!("Registry set_xcp_eth_params: {} {} {}", protocol_name, addr, port);
        self.xcp_tl_params = Some(McXcpTransportLayer {
            protocol_name,
            addr: Some(addr),
            port: Some(port),
            baud_rate: None,
        });
    }

    /// Set XCP transport layer parameters for SxI and enable XCP IF_DATA in A2L
    /// @param baud_rate: Baud rate of the SxI interface
    pub fn set_xcp_sxi_params(&mut self, baud_rate: u32) {
        log::debug!("Registry set_xcp_sxi_params: SxI {}", baud_rate);
        self.xcp_tl_params = Some(McXcpTransportLayer {
            protocol_name: "SxI",
            addr: None,
            port: None,
            baud_rate: Some(baud_rate),
        });
    }

    /// Check XCP transport layer information is available
    pub fn has_xcp_params(&self) -> bool {
        self.xcp_tl_params.is_some()
    }

    //---------------------------------------------------------------------------------------------------------
    // Modes

    /// Flatten typedefs (TYPEDEF_STRUCTURE) to measurement and calibration objects (MEASUREMENT, CHARACTERISTC  and AXIS) when writing A2L
    pub fn set_flatten_typedefs_mode(&mut self, flatten_typedefs: bool) {
        self.flatten_typedefs = flatten_typedefs;
    }
    pub fn get_flatten_typedefs_mode(&self) -> bool {
        self.flatten_typedefs
    }

    /// Prefix name with application name when writing A2L
    pub fn set_prefix_names_mode(&mut self, prefix_names: bool) {
        self.prefix_names = prefix_names;
    }
    pub fn get_prefix_names_mode(&self) -> bool {
        self.prefix_names
    }

    //---------------------------------------------------------------------------------------------------------
    // Typedefs

    /// Add a typedef component to a typedef
    pub fn add_typedef_field<T: Into<McIdentifier>>(
        &mut self,
        type_name: &str,
        field_name: T,
        dim_type: McDimType,
        mc_support_data: McSupportData,
        offset: u16,
    ) -> Result<(), RegistryError> {
        let field_name = field_name.into();
        log::debug!("Registry add_typedef_field: {}.{} dim_type={} offset={}", type_name, field_name, dim_type, offset);

        if let Some(typedef) = self.typedef_list.find_typedef_mut(type_name) {
            // Field already exists. This happens when the same struct type is registered
            // again (e.g. reused by multiple fields or instances). Verify the redefinition
            // is structurally identical (same offset and dimensional type); warn only on a
            // genuine ABI/layout conflict.
            if let Some(existing) = typedef.find_field(&field_name) {
                if existing.offset != offset || existing.dim_type != dim_type {
                    log::warn!(
                        "Conflicting redefinition of field {}.{}: existing offset={} dim_type={}, new offset={} dim_type={}",
                        type_name,
                        field_name,
                        existing.offset,
                        existing.dim_type,
                        offset,
                        dim_type
                    );
                }
                return Err(RegistryError::Duplicate(field_name.to_string()));
            }
            typedef.add_field(field_name, dim_type, mc_support_data, offset)
        } else {
            Err(RegistryError::NotFound(type_name.to_string()))
        }
    }

    /// Add a typedef
    pub fn add_typedef<T: Into<McIdentifier>>(&mut self, type_name: T, size: usize) -> Result<&mut McTypeDef, RegistryError> {
        let type_name = type_name.into();
        log::debug!("Registry add_typedef: {} size={}", type_name, size);

        // Note: no `is_closed()` guard here. `add_typedef` operates on `&mut self`,
        // which may be a standalone registry (e.g. the test client loading an uploaded
        // A2L after the singleton has been closed). The "no mutation after close" rule
        // is enforced at the singleton-access layer (`get_lock`), not on the instance.

        // Ignore if type name already exists
        // No separate name spaces for measurement and characteristic
        for t1 in &self.typedef_list {
            if *t1.name == *type_name {
                // Same struct type registered again (e.g. reused by multiple fields or
                // instances). Keep the existing definition. The per-field re-adds in
                // `add_typedef_field` validate structural equality; here we only flag a
                // mismatching overall size.
                if t1.size != size {
                    log::warn!("Conflicting redefinition of typedef {}: existing size={}, new size={}", type_name, t1.size, size);
                } else {
                    log::debug!("Duplicate typedef name {}, keeping existing definition", type_name);
                }
                return Err(RegistryError::Duplicate(type_name.to_string()));
            }
        }

        // Add to typedef list
        self.typedef_list.push(McTypeDef::new(type_name, size));
        let index = self.typedef_list.len() - 1;
        Ok(self.typedef_list.get_mut(index))
    }

    //---------------------------------------------------------------------------------------------------------

    /// Collapses all typedefs to measurement and calibration objects with mangled names
    pub fn flatten_typedefs(&mut self) {
        flatten_registry(self);
    }

    //---------------------------------------------------------------------------------------------------------
    // Set support data on a typedef field reachable from a named instance

    /// Set `McSupportData` on the typedef field identified by `instance_name` and a dot-separated
    /// `field_path` that navigates through nested typedefs.
    ///
    /// # Path syntax
    /// Each path component is a field name at the current typedef level:
    /// - `"kp"` — a direct field of the instance typedef
    /// - `"gains.kp"` — field `kp` inside the nested typedef reached via field `gains`
    ///
    /// # Metadata sharing note
    /// Typedef fields are shared across all instances of the same type.
    /// Calling this method on a field that appears in multiple instances will affect all of them.
    ///
    /// # Errors
    /// - `RegistryError::NotFound` — instance not found, instance is not a typedef, or a path
    ///   component does not exist
    /// - `RegistryError::MetadataAlreadySet` — the target field already has descriptive metadata
    pub fn set_instance_field_support_data(&mut self, instance_name: &str, field_path: &str, mut support_data: McSupportData) -> Result<(), RegistryError> {
        // 1. Resolve the top-level typedef name from the instance
        let top_typedef = self
            .instance_list
            .get_instance(instance_name, McObjectType::Unspecified, None)
            .and_then(|inst| inst.get_typedef_name())
            .ok_or_else(|| RegistryError::NotFound(instance_name.to_string()))?;

        let parts: Vec<&str> = field_path.split('.').collect();
        if parts.is_empty() {
            return Err(RegistryError::NotFound(field_path.to_string()));
        }

        // 2. First pass (shared borrow): walk all intermediate components to reach the
        //    typedef that directly owns the target field.
        //    All typedef names are &'static str (McIdentifier is interned), so `current`
        //    outlives the shared borrow without issue.
        let mut current_typedef: &'static str = top_typedef;
        for &component in &parts[..parts.len() - 1] {
            current_typedef = self
                .typedef_list
                .find_typedef(current_typedef)
                .ok_or_else(|| RegistryError::NotFound(current_typedef.to_string()))?
                .find_field(component)
                .and_then(|f| f.get_typedef_name())
                .ok_or_else(|| RegistryError::NotFound(component.to_string()))?;
        }

        // 3. Second pass (mutable borrow): reach the target field and set the metadata
        let target_field_name = parts[parts.len() - 1];
        let field = self
            .typedef_list
            .find_typedef_mut(current_typedef)
            .ok_or_else(|| RegistryError::NotFound(current_typedef.to_string()))?
            .find_field_mut(target_field_name)
            .ok_or_else(|| RegistryError::NotFound(field_path.to_string()))?;

        if field.mc_support_data.has_metadata() {
            // Don't replace wholesale — merge field-by-field so that multiple
            // annotations (e.g. XCP_LIMITS + XCP_UNIT) can each update their
            // own slice of the metadata without clobbering the others.
            field.mc_support_data.merge_metadata(support_data);
            return Ok(());
        }

        // Preserve the existing object_type when the caller leaves it Unspecified
        if support_data.object_type == McObjectType::Unspecified {
            support_data.object_type = field.mc_support_data.object_type;
        }
        field.mc_support_data = support_data;
        Ok(())
    }

    // ---------------------------------------------------------------------------------------------------------
    // Update the calibration segment numbers from a mapping table
    pub fn update_cal_seg_mapping(&mut self, mapping: &HashMap<u16, u16>) {
        for segment in &mut self.cal_seg_list {
            if let Some(new_index) = mapping.get(&segment.get_index()) {
                segment.set_index(*new_index);
                info!("Update calibration segment index {} -> {}", segment.get_index(), new_index);
            }
        }

        // @@@@ XCPlite with absolute segment addressing mode needs no update
        // Update of ADDR_MODE_A2L not checked

        for instance in &self.instance_list {
            if instance.address.is_segment_relative() {
                // Not implemented
                unimplemented!();
            }
        }
    }

    // Update the event id from a map (used by xcpclient to update the event id after connecting to the ECU)
    pub fn update_event_mapping(&mut self, mapping: &HashMap<u16, u16>) {
        for event in &mut self.event_list {
            if let Some(new_id) = mapping.get(&event.get_id()) {
                info!("Update event {} id {} -> {}", event.get_name(), event.get_id(), new_id);
                event.set_id(*new_id);
            }
        }
        for instance in &mut self.instance_list {
            if instance.address.is_event_relative() {
                unimplemented!();
            }
            if instance.address.get_addr_mode().is_a2l() {
                // @@@@ TODO: Hardcoded XCPlite specific address encoding
                let addr = instance.address.get_raw_a2l_addr();
                if addr.0 >= 2 {
                    unimplemented!();
                    // let event_id: u16 = (addr.1 >> McAddress::XCP_ADDR_EXT_DYN_OFFSET_BITS) as u16;
                    // info!("Checking address update for {}: {}:0x{:08X} event_id={}", instance.get_name(), addr.0, addr.1, event_id);
                    // if let Some(new_id) = mapping.get(&event_id) {
                    //     let new_addr: u32 = ((*new_id as u32) << 16) | (addr.1 & 0xFFFF);
                    //     instance.address.set_raw_a2l_addr(addr.0, new_addr);
                    //     log::info!(
                    //         "XCPlite specific event id update in address of ‘{}‘: {}:0x{:08X} -> 0x{:08X}",
                    //         instance.get_name(),
                    //         addr.0,
                    //         addr.1,
                    //         new_addr
                    //     );
                    // }
                }
            }
        }
    }

    //---------------------------------------------------------------------------------------------------------
    // Read and write registry from or to JSON file

    /// Serialize registry to JSON file
    pub fn write_json<P: AsRef<std::path::Path>>(&self, path: &P) -> Result<(), std::io::Error> {
        let path: &std::path::Path = path.as_ref();
        log::info!("Write JSON file {}", path.display());
        let json_file = std::fs::File::create(path)?;
        let mut writer = std::io::BufWriter::new(json_file);
        let s = serde_json::to_string_pretty(&self).map_err(|e| std::io::Error::other(format!("serde_json::to_string failed: {}", e)))?;
        std::io::Write::write_all(&mut writer, s.as_ref())?;
        Ok(())
    }

    /// Deserialize registry from JSON file
    pub fn load_json<P: AsRef<std::path::Path>>(&mut self, path: &P) -> Result<(), std::io::Error> {
        let path: &std::path::Path = path.as_ref();
        log::info!("Load JSON file {}", path.display());
        let json_file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(json_file);
        let r: Registry = serde_json::from_reader(reader).map_err(|e| std::io::Error::other(format!("serde_json::from_reader failed: {}", e)))?;
        *self = r;
        Ok(())
    }
}
