//-----------------------------------------------------------------------------
// Module a2l_reader
// A2L reader for registry based on crate a2lfile
// Detects Vector A2L if project id is "VECTOR" and then assumes predefined record layouts and typedefs

// VECTOR mode:
// - Predefined conversion rule IDENTIY
// - Predefined conversion rule BOOL
// - Address format
// - Segment 'epk' ignored
// - Predefined record layout names U8, A_U8, M_U8, C_U8

use super::*;

use a2lfile::A2lObjectName;
#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

impl Registry {
    // Load (merge) the content of a a2lfile data structure into this registry
    pub fn load_a2lfile(&mut self, a2l_file: &a2lfile::A2lFile) -> Result<(), String> {
        registry_load_a2lfile(self, a2l_file)
    }
}

// Get registry value type from a2lfile datatype
fn get_value_type_from_datatype(datatype: a2lfile::DataType) -> McValueType {
    match datatype {
        a2lfile::DataType::Ubyte => McValueType::Ubyte,
        a2lfile::DataType::Sbyte => McValueType::Sbyte,
        a2lfile::DataType::Uword => McValueType::Uword,
        a2lfile::DataType::Sword => McValueType::Sword,
        a2lfile::DataType::Ulong => McValueType::Ulong,
        a2lfile::DataType::Slong => McValueType::Slong,
        a2lfile::DataType::AUint64 => McValueType::Ulonglong,
        a2lfile::DataType::AInt64 => McValueType::Slonglong,
        a2lfile::DataType::Float32Ieee => McValueType::Float32Ieee,
        a2lfile::DataType::Float64Ieee => McValueType::Float64Ieee,
        _ => {
            error!("Unknown datatype: {:?}", datatype);
            McValueType::Ubyte
        }
    }
}

// Get value type from predefined record layout name
// Most people use some predefined record layout names for basic types of CHARACTERISTIC or TYPDEF_CHARACTERISTIC
// Just add them here
fn get_value_type_from_record_layout(s: &str) -> McValueType {
    // Type from record layout name, consider predefined Vector types
    match s {
        // Vector CANape predefined record layout names
        "__UBYTE_Z" => McValueType::Ubyte,
        "__SBYTE_Z" => McValueType::Sbyte,
        "__UWORD_Z" => McValueType::Uword,
        "__SWORD_Z" => McValueType::Sword,
        "__ULONG_Z" => McValueType::Ulong,
        "__SLONG_Z" => McValueType::Slong,
        "__A_UINT64_Z" => McValueType::Ulonglong,
        "__A_INT64_Z" => McValueType::Slonglong,
        "__FLOAT32_IEEE" => McValueType::Float32Ieee,
        "__FLOAT64_IEEE" => McValueType::Float64Ieee,

        // Vector VIO
        "_SBYTE" => McValueType::Sbyte,
        "_SLONG" => McValueType::Slong,
        "_SWORD" => McValueType::Sword,
        "_UBYTE" => McValueType::Ubyte,
        "_ULONG" => McValueType::Ulong,
        "_UWORD" => McValueType::Uword,

        // xcp-lite predefined record layout names
        "U8" => McValueType::Ubyte,
        "U16" => McValueType::Uword,
        "U32" => McValueType::Ulong,
        "U64" => McValueType::Ulonglong,
        "S8" => McValueType::Sbyte,
        "S16" => McValueType::Sword,
        "S32" => McValueType::Slong,
        "S64" => McValueType::Slonglong,
        "I8" => McValueType::Sbyte,
        "I16" => McValueType::Sword,
        "I32" => McValueType::Slong,
        "I64" => McValueType::Slonglong,
        "F32" => McValueType::Float32Ieee,
        "F64" => McValueType::Float64Ieee,
        "BOOL" => McValueType::Bool,
        "A_U8" => McValueType::Ubyte,
        "A_U16" => McValueType::Uword,
        "A_U32" => McValueType::Ulong,
        "A_U64" => McValueType::Ulonglong,
        "A_S8" => McValueType::Sbyte,
        "A_S16" => McValueType::Sword,
        "A_S32" => McValueType::Slong,
        "A_S64" => McValueType::Slonglong,
        "A_I8" => McValueType::Sbyte,
        "A_I16" => McValueType::Sword,
        "A_I32" => McValueType::Slong,
        "A_I64" => McValueType::Slonglong,
        "A_F32" => McValueType::Float32Ieee,
        "A_F64" => McValueType::Float64Ieee,

        // Source ?
        "VALUE_UBYTE" => McValueType::Ubyte,
        "VALUE_SBYTE" => McValueType::Sbyte,
        "VALUE_UWORD" => McValueType::Uword,
        "VALUE_SWORD" => McValueType::Sword,
        "VALUE_ULONG" => McValueType::Ulong,
        "VALUE_SLONG" => McValueType::Slong,
        "VALUE_A_UINT64" => McValueType::Ulonglong,
        "VALUE_A_INT64" => McValueType::Slonglong,
        "VALUE_FLOAT32_IEEE" => McValueType::Float32Ieee,
        "VALUE_FLOAT64_IEEE" => McValueType::Float64Ieee,

        _ => {
            // @@@@ TODO Suggestion: Record layout not predefined, add as typedef
            warn!("Unknown predefined record layout name '{}'", s);
            McValueType::Ubyte
        }
    }
}

fn get_conversion(conversion: &str, module: &a2lfile::Module, vector_xcp_mode: bool) -> (Option<f64>, Option<f64>) {
    if conversion != "NO_COMPU_METHOD" && conversion != "IDENTITY" {
        let opt_compu_method = module.compu_method.get(conversion);
        if let Some(compu_method) = opt_compu_method {
            let name = compu_method.get_name();
            match compu_method.conversion_type {
                a2lfile::ConversionType::Identical => (None, None),
                a2lfile::ConversionType::Linear => {
                    let coeffs_linear = compu_method.coeffs_linear.as_ref().unwrap();
                    (Some(coeffs_linear.a), Some(coeffs_linear.b))
                }
                a2lfile::ConversionType::RatFunc => {
                    let coeffs = compu_method.coeffs.as_ref().unwrap();
                    if coeffs.f != 0.0 && coeffs.d == 0.0 && coeffs.e == 0.0 && coeffs.a == 0.0 {
                        (Some(coeffs.b / coeffs.f), Some(coeffs.c))
                    } else {
                        debug!("RatFunc conversion not supported: {:?}", coeffs);
                        (None, None)
                    }
                }
                a2lfile::ConversionType::TabVerb => {
                    if vector_xcp_mode && name == "BOOL" {
                        (None, None) // Predefine conversion for BOOL, just ignore
                    } else {
                        // @@@@ TODO Implement TabVerb conversion
                        debug!("Compu method COMPU_VTAB not supported: {}", name);
                        (None, None)
                    }
                }

                _ => {
                    debug!("Compu method not supported: {:?}", compu_method.conversion_type);
                    (None, None)
                }
            }
        } else {
            error!("Compu method not found: {}", conversion);
            (None, None)
        }
    } else {
        (None, None)
    }
}

// Get event number from IF_DATA
fn get_event_id_from_ifdata(if_data_vec: &Vec<a2lfile::IfData>) -> Option<u16> {
    let mut event_id = None;
    for ifdata in if_data_vec {
        let decoded_ifdata = aml_ifdata::A2mlVector::load_from_ifdata(ifdata).unwrap();
        if let Some(xcp) = decoded_ifdata.xcp {
            if let Some(daq_event) = xcp.daq_event {
                if let Some(fixed_event_list) = daq_event.fixed_event_list {
                    event_id = Some(fixed_event_list.event[0].item);
                }
            }
        }
    }
    event_id
}

// Always read to A2L address representation
fn get_mc_address(registry: &Registry, addr: u32, addr_ext: u8, event_id: Option<u16>, vector_xcp_mode: bool) -> McAddress {
    debug!("Get address {:08X} ext {} event_id {:?}", addr, addr_ext, event_id);
    if !vector_xcp_mode {
        // Event id can only be stored in address
        if let Some(event_id) = event_id {
            McAddress::new_a2l_with_event(event_id, addr, addr_ext)
        } else {
            McAddress::new_a2l(addr, addr_ext)
        }
    } else {
        match addr_ext {
            McAddress::XCP_ADDR_EXT_SEG => {
                if let Some(calseg_name) = registry.cal_seg_list.find_cal_seg_by_address(addr) {
                    let offset: u16 = (addr & 0xFFFF) as u16;
                    McAddress::new_calseg_rel(calseg_name, offset as i32)
                } else {
                    error!("Calibration segment not found for address: {:08X}", addr);
                    McAddress::new_a2l(addr, addr_ext)
                }
            }
            McAddress::XCP_ADDR_EXT_DYN => {
                let event_id = event_id.unwrap();
                let addr_offset: i16 = (addr & 0xFFFF) as i16;
                assert_eq!(event_id, (addr >> 16) as u16);
                McAddress::new_event_dyn(event_id, addr_offset)
            }
            McAddress::XCP_ADDR_EXT_REL => {
                let event_id = event_id.unwrap();
                let addr_offset: i32 = addr as i32;
                McAddress::new_event_rel(event_id, addr_offset)
            }
            _ => {
                warn!("Address extension {} not supported", addr_ext);
                McAddress::new_a2l(addr, addr_ext)
            }
        }
    }
}

// Get matrix dimension as tuple
fn get_matrix_dim(matrix_dim: Option<&a2lfile::MatrixDim>) -> (u16, u16) {
    if let Some(m) = matrix_dim {
        if m.dim_list.len() >= 3 {
            error!("Matrix dimension {} >2 is not supported", m.dim_list.len());
            (1, 1)
        } else if m.dim_list.len() >= 2 {
            (m.dim_list[0], m.dim_list[1])
        } else if m.dim_list.len() == 1 {
            (m.dim_list[0], 1)
        } else {
            (1, 1)
        }
    } else {
        (1, 1)
    }
}

// Update any TYPEDEF_COMPONENT with a TYPEDEF_MEASUREMENT, TYPEDEF_CHARACTERISTIC or TYPEDEF_AXIS
// This will result in a leaf node in the registry typedef structure
#[allow(clippy::too_many_arguments)]
fn update_typedef_component(
    registry: &mut Registry,
    typedef_name: &str,
    object_type: McObjectType,
    value_type: McValueType,
    matrix_dim: Option<&a2lfile::MatrixDim>,
    comment: &String,
    min: f64,
    max: f64,
    unit: Option<&String>,
    factor: Option<f64>,
    offset: Option<f64>,
) {
    for typedef in registry.typedef_list.iter_mut() {
        for field in typedef.fields.iter_mut() {
            if let McValueType::TypeDef(type_name) = field.dim_type.value_type {
                if type_name == typedef_name {
                    field.dim_type.value_type = value_type;
                    let dim = get_matrix_dim(matrix_dim);
                    field.dim_type.x_dim = Some(dim.0);
                    field.dim_type.y_dim = Some(dim.1);
                    let mc_support_data = McSupportData::new(object_type)
                        .set_comment(comment)
                        .set_min(Some(min))
                        .set_max(Some(max))
                        .set_factor(factor)
                        .set_offset(offset)
                        .set_unit(unit);
                    field.mc_support_data = mc_support_data;
                    log::debug!("Update typedef component {} with type {}", field.name, typedef_name);
                }
            }
        }
    }
}

fn registry_load_a2lfile(registry: &mut Registry, a2l_file: &a2lfile::A2lFile) -> Result<(), String> {
    let mut vector_xcp_mode: bool = false;

    info!("Load A2L file into registry:");
    if let Some(a2l_version) = a2l_file.asap2_version.as_ref() {
        debug!("  A2l Version: {}.{}", a2l_version.version_no, a2l_version.upgrade_no);
        debug!("  AML Version: {:?}", a2l_file.a2ml_version);
    } else {
        error!("A2l Version not found");
        return Err("A2l Version not found".to_string());
    }
    debug!("  Projectname: {}", a2l_file.project.name);
    debug!("  Project description: {}", a2l_file.project.long_identifier);
    if let Some(header) = a2l_file.project.header.as_ref() {
        debug!("  Header comment: {}", header.comment);
        if let Some(header_version) = header.version.as_ref() {
            debug!("  Header version: {}", header_version.version_identifier);
        }
        if let Some(project_no) = header.project_no.as_ref() {
            debug!("  Header project number: {}", project_no.project_number);
            vector_xcp_mode = project_no.project_number == "VECTOR";
        }
    };
    debug!("  Modulename: {}", a2l_file.project.module[0].get_name());
    info!("Vector A2L mode = {:?}", vector_xcp_mode);

    // If this A2l has been witten by the xcp-lite registry, assume xcp-lite addressing modes and predefined record layouts and typedef

    registry.set_app_info(a2l_file.project.name.clone(), format!("created from a2lfile project {}", a2l_file.project.name), 0);
    registry.set_vector_xcp_mode(vector_xcp_mode);

    let module = &a2l_file.project.module[0];

    //----------------------------------------------------------------------------------------------------------------
    // Memory segments

    // Add memory segments and EPK
    let mut index = 0;
    if let Some(mod_par) = &module.mod_par {
        // EPK
        if let Some(epk) = &mod_par.epk {
            let version_epk = epk.identifier.clone();
            let version_addr = if !mod_par.addr_epk.is_empty() { mod_par.addr_epk[0].address } else { 0 };
            debug!("Set EPK: {} {:08X}", version_epk, version_addr);
            registry.set_app_version(version_epk, version_addr);
        }

        // Memory segments
        for m in mod_par.memory_segment.iter() {
            let name = m.get_name().to_string();
            let addr_ext = 0; // @@@@ xcp-lite address extensions hardcoded here, would be in IF_DATA
            let addr = m.address;
            let size = m.size;
            if vector_xcp_mode {
                if name == "epk" {
                    // Predefined memory segment for EPK, just ignore
                    continue;
                }
                // Get index from addr
                index = ((addr >> 16) & 0x7FFF) as u16 - 1; // EPK segment is virtual, index starts with 0
            } else {
                index += 1; // Index starts with 1, 0 is predefined EPK segment
            }

            let res = registry.cal_seg_list.add_a2l_cal_seg(name, index, addr_ext, addr, size);
            match res {
                Ok(_) => {}
                Err(e) => {
                    warn!("Failed to add calibration segment: {}", e);
                }
            }
        }
    }

    //----------------------------------------------------------------------------------------------------------------
    // Characteristics

    // Add characteristics
    for characteristic in &module.characteristic {
        let name = characteristic.get_name().to_string();
        let value_type = get_value_type_from_record_layout(&characteristic.deposit);
        let conversion = &characteristic.conversion;
        let (factor, offset) = get_conversion(conversion, module, vector_xcp_mode);
        let (x_dim, y_dim) = get_matrix_dim(characteristic.matrix_dim.as_ref());
        let object_type: McObjectType = McObjectType::Characteristic;
        match characteristic.characteristic_type {
            a2lfile::CharacteristicType::Cuboid => {
                panic!("Cuboid not supported");
            }
            a2lfile::CharacteristicType::Cube4 => {
                panic!("Cube4 not supported");
            }
            a2lfile::CharacteristicType::Cube5 => {
                panic!("Cube5 not supported");
            }
            _ => {}
        }

        // Metadata
        let mut mc_support_data = McSupportData::new(object_type)
            .set_comment(characteristic.long_identifier.clone())
            .set_min(Some(characteristic.lower_limit))
            .set_max(Some(characteristic.upper_limit))
            .set_factor(factor)
            .set_offset(offset);
        if let Some(u) = characteristic.phys_unit.as_ref() {
            mc_support_data = mc_support_data.set_unit(u.unit.clone());
        }

        // Axis refs
        if !characteristic.axis_descr.is_empty() {
            if let Some(r) = characteristic.axis_descr[0].axis_pts_ref.as_ref() {
                let s = r.axis_points.clone();
                mc_support_data = mc_support_data.set_x_axis_ref(Some(s));
                if characteristic.axis_descr.len() > 1 {
                    if let Some(r) = characteristic.axis_descr[1].axis_pts_ref.as_ref() {
                        let s = r.axis_points.clone();
                        mc_support_data = mc_support_data.set_y_axis_ref(Some(s));
                    }
                }
            }
        }

        // Dimension and type
        let dim_type = McDimType::new(value_type, x_dim, y_dim);

        // Address
        let event_id = get_event_id_from_ifdata(&characteristic.if_data);
        if event_id.is_some() {
            info!("Characteristic {} has event id {}", name, event_id.unwrap());
        }
        let addr_ext = characteristic.ecu_address_extension.as_ref().map(|a| a.extension).unwrap_or(0) as u8;
        let addr = characteristic.address;
        let address = get_mc_address(registry, addr, addr_ext, event_id, vector_xcp_mode);

        // Add characteristic instance
        let res = registry.instance_list.add_instance(name, dim_type, mc_support_data, address);
        match res {
            Ok(_) => {}
            Err(e) => {
                warn!("Failed to add characteristic instance: {}", e);
            }
        }
    } // for characteristic

    //----------------------------------------------------------------------------------------------------------------
    // Measurements

    // Add measurements
    for measurement in &module.measurement {
        let name = measurement.get_name().to_string();
        let _read_write = measurement.read_write.is_some();

        let datatype = measurement.datatype;
        let lower_limit = measurement.lower_limit;
        let upper_limit = measurement.upper_limit;
        let _display_identifier = &measurement.display_identifier;
        let _resolution = measurement.resolution;

        // Conversion
        let conversion = &measurement.conversion;
        let phys_unit = &measurement.phys_unit;
        let (factor, offset) = get_conversion(conversion, module, vector_xcp_mode);

        // Dimension
        let matrix_dim = &measurement.matrix_dim;
        let (x_dim, y_dim) = get_matrix_dim(matrix_dim.as_ref());

        let object_type: McObjectType = McObjectType::Measurement;
        let value_type = get_value_type_from_datatype(datatype);

        // Metadata
        let mut mc_support_data = McSupportData::new(object_type)
            // @@@@ TODO find a solution for the clones, Rust does not allow different trait implementations for &str in into McText for different lifetimes
            .set_comment(measurement.long_identifier.clone())
            .set_min(Some(lower_limit))
            .set_max(Some(upper_limit))
            .set_factor(factor)
            .set_offset(offset);
        if let Some(u) = phys_unit.as_ref() {
            mc_support_data = mc_support_data.set_unit(u.unit.clone());
        }

        // Dimension and type
        let dim_type = McDimType::new(value_type, x_dim, y_dim);

        // Address
        let event_id = get_event_id_from_ifdata(&measurement.if_data);
        if event_id.is_none() {
            warn!("Measurement {} has no event id", name);
        }
        let addr = if let Some(a) = measurement.ecu_address.as_ref() { a.address } else { 0 };
        let addr_ext = if let Some(addr_ext) = &measurement.ecu_address_extension {
            addr_ext.extension as u8
        } else {
            0
        };
        let address = get_mc_address(registry, addr, addr_ext, event_id, vector_xcp_mode);

        // Add measurement instance
        let res = registry.instance_list.add_instance(name, dim_type, mc_support_data, address);
        match res {
            Ok(_) => {}
            Err(e) => {
                warn!("Failed to add measurement instance: {}", e);
            }
        }
    } // for measurement

    //----------------------------------------------------------------------------------------------------------------
    // Axis

    //----------------------------------------------------------------------------------------------------------------
    // Typedefs

    for typedef in &module.typedef_structure {
        let typedef_name = typedef.get_name().to_string();
        // Add typedef
        let res = registry.add_typedef(typedef_name, typedef.total_size as usize);
        match res {
            Ok(_) => {}
            Err(e) => {
                warn!("Failed to add typedef: {}", e);
            }
        }

        // Add typedef components
        for field in &typedef.structure_component {
            let typedef_name = typedef.get_name();
            let field_name = field.get_name().to_string();
            // Dimension
            let matrix_dim = &field.matrix_dim;
            let (x_dim, y_dim) = get_matrix_dim(matrix_dim.as_ref());

            // Add typedef field
            let offset = field.address_offset as u16;
            let value_type = McValueType::TypeDef(field.component_type.clone().into());
            let mc_support_data = McSupportData::new(McObjectType::Unspecified);
            let dim_type = McDimType::new(value_type, x_dim, y_dim);
            let res = registry.add_typedef_component(typedef_name, field_name, dim_type, mc_support_data, offset);
            match res {
                Ok(_) => {}
                Err(e) => {
                    warn!("Failed to add typedef field: {}", e);
                }
            }
        }
    }

    for typedef in &module.typedef_measurement {
        let name = typedef.get_name();
        let conversion = &typedef.conversion;
        let (factor, offset) = get_conversion(conversion, module, vector_xcp_mode);
        let comment = &typedef.long_identifier;
        let min = typedef.lower_limit;
        let max = typedef.upper_limit;
        let unit = typedef.phys_unit.as_ref().map(|u| &u.unit);
        let value_type = get_value_type_from_datatype(typedef.datatype);
        update_typedef_component(
            registry,
            name,
            McObjectType::Measurement,
            value_type,
            typedef.matrix_dim.as_ref(),
            comment,
            min,
            max,
            unit,
            factor,
            offset,
        );
    }
    for typedef in &module.typedef_characteristic {
        let name = typedef.get_name();
        let conversion = &typedef.conversion;
        let (factor, offset) = get_conversion(conversion, module, vector_xcp_mode);
        let record_layout = &typedef.record_layout;
        let comment = &typedef.long_identifier;
        let min = typedef.lower_limit;
        let max = typedef.upper_limit;
        let unit = typedef.phys_unit.as_ref().map(|u| &u.unit);
        let value_type = get_value_type_from_record_layout(record_layout);
        let _sub_type = typedef.characteristic_type; // ValBlk, Value, Ascii, Curve, Cuboid, Cube4, Cube5
        update_typedef_component(
            registry,
            name,
            McObjectType::Characteristic,
            value_type,
            typedef.matrix_dim.as_ref(),
            comment,
            min,
            max,
            unit,
            factor,
            offset,
        );
    }
    for typedef in &module.typedef_axis {
        let name = typedef.get_name();
        let conversion = &typedef.conversion;
        let (factor, offset) = get_conversion(conversion, module, vector_xcp_mode);
        let record_layout = &typedef.record_layout;
        let comment = &typedef.long_identifier;
        let min = typedef.lower_limit;
        let max = typedef.upper_limit;
        let unit = typedef.phys_unit.as_ref().map(|u| &u.unit);
        let value_type = get_value_type_from_record_layout(record_layout);
        update_typedef_component(registry, name, McObjectType::Axis, value_type, None, comment, min, max, unit, factor, offset);
    }

    //----------------------------------------------------------------------------------------------------------------
    // Instances

    // Add instances
    // Flatten typedefs optionally
    for instance in &module.instance {
        let name = instance.get_name().to_string();

        // Dimension
        let matrix_dim = &instance.matrix_dim;
        let (x_dim, y_dim) = if let Some(m) = matrix_dim.as_ref() {
            if m.dim_list.len() >= 3 {
                error!("Matrix dimension {} >2 is not supported", m.dim_list.len());
                (1, 1)
            } else if m.dim_list.len() >= 2 {
                (m.dim_list[0], m.dim_list[1])
            } else if m.dim_list.len() == 1 {
                (m.dim_list[0], 1)
            } else {
                (1, 1)
            }
        } else {
            (1, 1)
        };

        // Event id
        // Measurement or characteristic object type depending on existance of event id
        let event_id = get_event_id_from_ifdata(&instance.if_data);
        let object_type: McObjectType = if event_id.is_none() { McObjectType::Characteristic } else { McObjectType::Measurement };

        // Metadata, dimension and type
        let comment = &instance.long_identifier;
        let value_type = McValueType::TypeDef(instance.type_ref.clone().into());
        let mc_support_data = McSupportData::new(object_type).set_comment(comment.clone());
        let dim_type = McDimType::new(value_type, x_dim, y_dim);

        // Address
        let addr = instance.start_address;
        let addr_ext = if let Some(addr_ext) = &instance.ecu_address_extension {
            addr_ext.extension as u8
        } else {
            0
        };
        let address = get_mc_address(registry, addr, addr_ext, event_id, vector_xcp_mode);

        // Add instance of a typedef
        let res = registry.instance_list.add_instance(name, dim_type, mc_support_data, address);
        match res {
            Ok(_) => {}
            Err(e) => {
                warn!("Failed to add typedef instance: {}", e);
            }
        }
    }

    Ok(())
}
