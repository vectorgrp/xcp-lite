// Module mc_registry
// Types:
//  Registry

use serde::Deserialize;
use serde::Serialize;
use std::net::Ipv4Addr;

use super::is_closed;

use super::McCalibrationSegmentList;
use super::McDimType;
use super::McEventList;
use super::McIdentifier;
use super::McInstanceList;
use super::McText;
use super::McTypeDef;
use super::McTypeDefList;
use super::McXcpTransportLayer;
use super::RegistryError;
use super::flatten_registry;

//-------------------------------------------------------------------------------------------------
// ApplicationVersion
// EPK software version id

#[derive(Debug, Serialize, Deserialize)]
struct ApplicationVersion {
    epk: McText,
    version_addr: u32,
}

impl ApplicationVersion {
    fn new() -> ApplicationVersion {
        ApplicationVersion::default()
    }
}

impl Default for ApplicationVersion {
    fn default() -> Self {
        ApplicationVersion {
            epk: "EPK_".into(),
            version_addr: 0,
        }
    }
}

//-------------------------------------------------------------------------------------------------
// Application

/// Infos on the application
#[derive(Debug, Default, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize)]
pub struct McApplication {
    pub app_id: u8,          // Unique identifier for the application
    pub name: McIdentifier,  // Name of the application, used as A2L filename and module name
    pub description: McText, // Optional description of the application
    // Version or EPK
    pub version: McText,   // Version, used as A2L EPK
    pub version_addr: u32, // Address of the EPK string in memory
}

//-------------------------------------------------------------------------------------------------
// Registry

/// Measurement and calibration object database
#[derive(Debug, Serialize, Deserialize)]
pub struct Registry {
    // Use Vector specific extensions
    // XCP definition of address extensions
    vector_xcp_mode: bool,

    // Flatten typedefs to measurement and calibration objects when writing A2L
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    pub flatten_typedefs: bool,

    // Prefix name wit application name when writing A2L
    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    pub prefix_names: bool,

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
            vector_xcp_mode: true,
            flatten_typedefs: false,
            prefix_names: false,
            application: McApplication::default(),
            xcp_tl_params: None,
            event_list: McEventList::new(),
            cal_seg_list: McCalibrationSegmentList::new(),
            typedef_list: McTypeDefList::new(),
            instance_list: McInstanceList::new(),
        }
    }

    //---------------------------------------------------------------------------------------------------------

    /// Set application name
    pub fn set_app_info<A: Into<McIdentifier>, B: Into<McText>>(&mut self, name: A, description: B, id: u8) {
        let name: McIdentifier = name.into();
        let description: McText = description.into();
        log::info!("Registry set application info, app_name={}, app_id={}, description={}", name, id, description);

        // Set name, id and description
        self.application.app_id = id;
        self.application.name = name;
        self.application.description = description;
    }

    /// Get application name
    pub fn get_app_name(&self) -> &'static str {
        self.application.name.as_str()
    }

    /// Set application version
    pub fn set_app_version<T: Into<McText>>(&mut self, epk: T, version_addr: u32) {
        let epk: McText = epk.into();
        log::debug!("Registry set epk: {} 0x{:08X}", epk, version_addr);
        self.application.version = epk;
        self.application.version_addr = version_addr;
    }

    /// Get application version
    pub fn get_app_version(&self) -> &str {
        self.application.version.as_str()
    }

    //---------------------------------------------------------------------------------------------------------
    // XCP parameters (ID_DATA XCP)

    /// Set XCP transport layer parameters and enable XCP IF_DATA in A2L
    pub fn set_xcp_params(&mut self, protocol_name: &'static str, addr: Ipv4Addr, port: u16) {
        log::debug!("Registry set_xcp_tl_params: {} {} {}", protocol_name, addr, port);
        self.xcp_tl_params = Some(McXcpTransportLayer { protocol_name, addr, port });
    }

    /// Check XCP transport layer information is available
    pub fn has_xcp_params(&self) -> bool {
        self.xcp_tl_params.is_some()
    }

    /// Vector specific handling mode of address extensions, predefined artefacts, naming conventions, usage of typedefs, ...
    pub fn set_vector_xcp_mode(&mut self, vector_xcp_mode: bool) {
        self.vector_xcp_mode = vector_xcp_mode;
    }
    pub fn is_vector_xcp_mode(&self) -> bool {
        self.vector_xcp_mode
    }

    /// Flatten typedefs (TYPEDEF_STRUCTURE) to measurement and calibration objects (MEASUREMENT, CHARACTERISTC  and AXIS) when writing A2L
    pub fn set_flatten_typedefs(&mut self, flatten_typedefs: bool) {
        self.flatten_typedefs = flatten_typedefs;
    }

    /// Prefix name with application name when writing A2L
    pub fn set_prefix_names(&mut self, prefix_names: bool) {
        self.prefix_names = prefix_names;
    }

    //---------------------------------------------------------------------------------------------------------
    // Typedefs

    /// Add a typedef component to a typedef
    pub fn add_typedef_component<T: Into<McIdentifier>>(&mut self, type_name: &str, field_name: T, dim_type: McDimType, offset: u16) -> Result<(), RegistryError> {
        let field_name = field_name.into();
        log::debug!("Registry add_typedef_component: {}.{} dim_type={} offset={}", type_name, field_name, dim_type, offset);

        if let Some(typedef) = self.typedef_list.find_typedef_mut(type_name) {
            // Duplicate field name
            if typedef.find_field(&field_name).is_some() {
                return Err(RegistryError::Duplicate(field_name.to_string()));
            }
            typedef.add_field(field_name, dim_type, offset)
        } else {
            Err(RegistryError::NotFound(type_name.to_string()))
        }
    }

    /// Add a typedef
    pub fn add_typedef<T: Into<McIdentifier>>(&mut self, type_name: T, size: usize) -> Result<&mut McTypeDef, RegistryError> {
        let type_name = type_name.into();
        log::debug!("Registry add_typedef: {} size={}", type_name, size);

        // Panic if registry is closed
        assert!(!is_closed(), "Registry is closed");

        // Ignore if type name name already exists
        // No separate name spaces for measurement and characteristic
        for t1 in &self.typedef_list {
            if *t1.name == *type_name {
                log::warn!("Duplicate typedef name {}, equality not checked!", type_name);
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
