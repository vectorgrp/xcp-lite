//-----------------------------------------------------------------------------
// Module a2l_writer
// Export an A2L file from super::registry

// Use log_level=Debug to track what exactly is written

use std::{collections::HashMap, io::Write};

use super::*;

trait GenerateA2l {
    fn write_a2l(&self, writer: &mut A2lWriter) -> std::io::Result<()>;
}

// A2L types as str
impl McValueType {
    // Basic type
    fn get_type_str(&self) -> &'static str {
        match self {
            McValueType::Bool => "UBYTE",
            McValueType::Ubyte => "UBYTE",
            McValueType::Uword => "UWORD",
            McValueType::Ulong => "ULONG",
            McValueType::Ulonglong => "A_UINT64",
            McValueType::Sbyte => "SBYTE",
            McValueType::Sword => "SWORD",
            McValueType::Slong => "SLONG",
            McValueType::Slonglong => "A_INT64",
            McValueType::Float32Ieee => "FLOAT32_IEEE",
            McValueType::Float64Ieee => "FLOAT64_IEEE",
            McValueType::Blob(_) => "BLOB",
            McValueType::TypeDef(_) => panic!("get_type_str: Instance not allowed as measurement"),
            McValueType::Unknown => panic!("get_type_str: Unknown type"),
        }
    }

    // Get data type as str representing the A2L record_layout (all are predefined record layouts)
    // Used by A2L writer
    fn get_record_layout_str(&self) -> &'static str {
        match self {
            McValueType::Bool => "BOOL",
            McValueType::Ubyte => "U8",
            McValueType::Uword => "U16",
            McValueType::Ulong => "U32",
            McValueType::Ulonglong => "U64",
            McValueType::Sbyte => "I8",
            McValueType::Sword => "I16",
            McValueType::Slong => "I32",
            McValueType::Slonglong => "I64",
            McValueType::Float32Ieee => "F32",
            McValueType::Float64Ieee => "F64",
            McValueType::Blob(_) => "BLOB",
            McValueType::TypeDef(_) => panic!("get_record_layout_str: Instance not allowed as record_layout"),
            _ => panic!("get_record_layout_str: Unsupported data type"),
        }
    }
}

// Get the A2L object type of the calibration parameter
fn get_characteristic_subtype_str(dim_type: &McDimType) -> &'static str {
    let mc_support_data = &dim_type.mc_support_data;

    match mc_support_data.object_type {
        McObjectType::Axis => "AXIS_PTS",
        McObjectType::Characteristic => {
            if dim_type.get_dim()[0] > 1
                && mc_support_data.x_axis_ref.is_none()
                && mc_support_data.y_axis_ref.is_none()
                && mc_support_data.x_axis_conv.is_none()
                && mc_support_data.y_axis_conv.is_none()
            {
                "VAL_BLK"
            } else if dim_type.get_dim()[0] > 1 && dim_type.get_dim()[1] > 1 {
                "MAP"
            } else if dim_type.get_dim()[0] > 1 || dim_type.get_dim()[1] > 1 {
                "CURVE"
            } else {
                "VALUE"
            }
        }
        _ => panic!("get_characteristic_type_str: Unsupported object type {:?}", mc_support_data.object_type),
    }
}

//-------------------------------------------------------------------------------------------------

// Write a conversion rule and return its name as string
fn write_conversion<'a>(writer: &mut A2lWriter, name: &'a str, instance_index: u16, dim_type: &McDimType) -> std::io::Result<&'a str> {
    let factor = dim_type.get_factor().unwrap_or(1.0);
    let offset = dim_type.get_offset().unwrap_or(0.0);
    let unit = dim_type.get_unit();

    // Bool: Use BOOL
    if dim_type.value_type == McValueType::Bool {
        Ok("BOOL")
    }
    // Conversion
    // Write a conversion and return its name
    else if (factor - 1.0).abs() > f64::EPSILON || offset.abs() > f64::EPSILON {
        // For measurements with multiple tli instances, the conversion name is created only once on index 1
        if instance_index > 1 {
            return Ok(name);
        }

        /*
        display format in %[<length>].<layout>
        <length> is an optional unsigned integer value, which indicates the overall length;
        <layout> is a mandatory unsigned integer value, which indicates the decimal places;
        The format string must always contain at least "%", "." and <layout>.
        */
        let layout: u8 = if dim_type.value_type == McValueType::Float32Ieee || dim_type.value_type == McValueType::Float64Ieee || factor < 0.001 {
            6
        } else if factor < 1.0 {
            3
        } else {
            0
        };

        writeln!(
            writer,
            r#"/begin COMPU_METHOD {name} "" LINEAR "%.{layout}" "{unit}" COEFFS_LINEAR {factor} {offset} /end COMPU_METHOD"#
        )?;
        Ok(name)
    }
    // No conversion
    else {
        // Float: Use NO_COMPU_METHOD
        if dim_type.value_type == McValueType::Float32Ieee || dim_type.value_type == McValueType::Float64Ieee {
            Ok("NO_COMPU_METHOD")
        }
        // Integer: Use predefined conversion rule identit with .0 display format
        else {
            Ok("IDENTITY")
        }
    }
}

// Write MATRIX_DIM if multi-dimensional VALUE
fn write_dimensions(dim_type: &McDimType, writer: &mut A2lWriter) -> std::io::Result<()> {
    let x_dim = dim_type.get_dim()[0];
    let y_dim = dim_type.get_dim()[1];
    if x_dim > 1 && y_dim > 1 {
        write!(writer, " MATRIX_DIM {} {}", x_dim, y_dim)?;
    } else if x_dim > 1 {
        write!(writer, " MATRIX_DIM {}", x_dim)?;
    }
    Ok(())
}

// Write AXIS_DESCR for MAP or CURVE or TYPEDEF_MAP, TYPEDEF_CURVE
fn write_axis_descr(_name: &str, dim_type: &McDimType, writer: &mut A2lWriter) -> std::io::Result<()> {
    let x_dim = dim_type.get_dim()[0];
    let y_dim = dim_type.get_dim()[1];
    let mc_support_data = &dim_type.mc_support_data;

    // MAP
    if x_dim > 1 || y_dim > 1 {
        if x_dim > 1 && y_dim > 1 {
            // X
            if let Some(x_axis_conv) = &mc_support_data.x_axis_conv {
                write!(
                    writer,
                    r#" /begin AXIS_DESCR FIX_AXIS NO_INPUT_QUANTITY {} {} 0 {} FIX_AXIS_PAR_DIST 0 1 {} /end AXIS_DESCR"#,
                    x_axis_conv,
                    x_dim,     // MaxAxisPoints
                    x_dim - 1, // 0-UpperLimit
                    x_dim
                )?;
            } else if let Some(x_axis_ref) = &mc_support_data.x_axis_ref {
                write!(
                    writer,
                    r#" /begin AXIS_DESCR COM_AXIS NO_INPUT_QUANTITY NO_COMPU_METHOD {} 0.0 0.0 AXIS_PTS_REF {} /end AXIS_DESCR"#,
                    x_dim, x_axis_ref
                )?;
            } else {
                write!(
                    writer,
                    r#" /begin AXIS_DESCR FIX_AXIS NO_INPUT_QUANTITY NO_COMPU_METHOD {} 0 {} FIX_AXIS_PAR_DIST 0 1 {} /end AXIS_DESCR"#,
                    x_dim,
                    x_dim - 1,
                    x_dim
                )?;
            }
            // Y
            if let Some(y_axis_conv) = &mc_support_data.y_axis_conv {
                write!(
                    writer,
                    r#" /begin AXIS_DESCR FIX_AXIS NO_INPUT_QUANTITY {} {} 0 {} FIX_AXIS_PAR_DIST 0 1 {} /end AXIS_DESCR"#,
                    y_axis_conv,
                    y_dim,
                    y_dim - 1,
                    y_dim
                )?;
            } else if let Some(y_axis_ref) = &mc_support_data.y_axis_ref {
                write!(
                    writer,
                    r#" /begin AXIS_DESCR COM_AXIS NO_INPUT_QUANTITY NO_COMPU_METHOD {} 0.0 0.0 AXIS_PTS_REF {} /end AXIS_DESCR"#,
                    y_dim, y_axis_ref
                )?;
            } else {
                write!(
                    writer,
                    r#" /begin AXIS_DESCR FIX_AXIS NO_INPUT_QUANTITY NO_COMPU_METHOD {} 0 {} FIX_AXIS_PAR_DIST 0 1 {} /end AXIS_DESCR"#,
                    y_dim,
                    y_dim - 1,
                    y_dim
                )?;
            }
        }
        // CURVE
        else if x_dim > 1 {
            if let Some(x_axis_conv) = &mc_support_data.x_axis_conv {
                write!(
                    writer,
                    r#" /begin AXIS_DESCR FIX_AXIS NO_INPUT_QUANTITY {} {} 0 {} FIX_AXIS_PAR_DIST 0 1 {} /end AXIS_DESCR"#,
                    x_axis_conv,
                    x_dim,
                    x_dim - 1,
                    x_dim
                )?;
            } else if let Some(x_axis_ref) = &mc_support_data.x_axis_ref {
                write!(
                    writer,
                    r#" /begin AXIS_DESCR COM_AXIS NO_INPUT_QUANTITY NO_COMPU_METHOD {} 0.0 0.0 AXIS_PTS_REF {} /end AXIS_DESCR"#,
                    x_dim, x_axis_ref
                )?;
            } else {
                write!(
                    writer,
                    r#" /begin AXIS_DESCR FIX_AXIS NO_INPUT_QUANTITY NO_COMPU_METHOD {} 0 {} FIX_AXIS_PAR_DIST 0 1 {} /end AXIS_DESCR"#,
                    x_dim,
                    x_dim - 1,
                    x_dim
                )?;
            }
        }
    }

    Ok(())
}

// Write instance IF_DATA with event
fn write_ifdata_event(event_id: u16, writer: &mut A2lWriter) -> std::io::Result<()> {
    // Fixed event
    write!(writer, " /begin IF_DATA XCP /begin DAQ_EVENT FIXED_EVENT_LIST EVENT {event_id} /end DAQ_EVENT /end IF_DATA")?;
    Ok(())
}

//-------------------------------------------------------------------------------------------------

impl GenerateA2l for McXcpTransportLayer {
    fn write_a2l(&self, writer: &mut A2lWriter) -> std::io::Result<()> {
        let protocol = self.protocol_name.to_uppercase();
        let port = self.port;
        let addr = self.addr;

        log::debug!("A2L writer: transport layer: {protocol} {addr}:{port}");

        writeln!(writer, "\n\t\t\t/begin XCP_ON_{protocol}_IP 0x104 {port} ADDRESS \"{addr}\" /end XCP_ON_{protocol}_IP")
    }
}

//-------------------------------------------------------------------------------------------------

impl GenerateA2l for McEvent {
    fn write_a2l(&self, writer: &mut A2lWriter) -> std::io::Result<()> {
        let name = &self.name;
        let index = self.index;
        let id = self.id;

        log::debug!("A2L writer: event {} index={} id={}", name, index, id);

        let priority = 0; // Priority is always set to normal priority

        // Convert cycle time to ASAM coding timeCycle and timeUnit
        // "UNIT_1NS" = 0, "UNIT_10NS" = 1, ...
        // @@@@ TODO Create warning that not all cycle times can be represented in ASAM timeCycle and timeUnit
        let mut time_unit: u8 = 0;
        let mut time_cycle = self.target_cycle_time_ns;
        while time_cycle >= 256 {
            time_cycle /= 10;
            time_unit += 1;
        }

        // long name 100+1 characters
        // short name 8+1 characters
        // TimeCycle 0
        // TimeUnit 0
        // Priority 0
        // @@@@ TODO CANape does not accept CONSISTENCY EVENT for serialized data types !!!!!!!!!!
        if index > 0 {
            write!(writer, "/begin EVENT \"{:.98}_{}\" \"{:.6}_{}\" ", name, index, name, index)?;
        } else {
            write!(writer, "/begin EVENT \"{:.100}\" \"{:.8}\" ", name, name)?;
        }
        writeln!(writer, "{} DAQ 0xFF {} {} {} CONSISTENCY DAQ /end EVENT", id, time_cycle, time_unit, priority)
    }
}

//-------------------------------------------------------------------------------------------------

impl GenerateA2l for McApplication {
    fn write_a2l(&self, writer: &mut A2lWriter) -> std::io::Result<()> {
        // Add a EPK memory segment for the EPK, to include the EPK in HEX-files
        log::debug!("A2L writer: epk={} version_addr=0x{:08X}", self.version, self.version_addr);
        writeln!(
            writer,
            "EPK \"{}\" ADDR_EPK 0x{:08X}\n/begin MEMORY_SEGMENT epk \"\" DATA FLASH INTERN 0x{:08X} {} -1 -1 -1 -1 -1 /end MEMORY_SEGMENT",
            self.version,
            self.version_addr,
            self.version_addr,
            self.version.len(),
        )
    }
}

//-------------------------------------------------------------------------------------------------
// CalibrationSegment
// A2L MEMORY_SEGMENT

impl GenerateA2l for McCalibrationSegment {
    fn write_a2l(&self, writer: &mut A2lWriter) -> std::io::Result<()> {
        let calseg = self;
        let n = calseg.index;

        //for (n, calseg) in self.iter().enumerate() {
        log::debug!("A2L writer: memory segment {}  {}:0x{:X} size={}", calseg.name, calseg.addr_ext, calseg.addr, calseg.size);

        writeln!(
            writer,
            r#"/begin MEMORY_SEGMENT {} "" DATA FLASH INTERN 0x{:X} {} -1 -1 -1 -1 -1"#,
            calseg.name, calseg.addr, calseg.size,
        )?;

        writeln!(
            writer,
            r#"/begin IF_DATA XCP
    /begin SEGMENT {} 2 {} 0 0
    /begin CHECKSUM XCP_ADD_44 MAX_BLOCK_SIZE 0xFFFF EXTERNAL_FUNCTION "" /end CHECKSUM
    /begin PAGE 0x0 ECU_ACCESS_DONT_CARE XCP_READ_ACCESS_DONT_CARE XCP_WRITE_ACCESS_DONT_CARE /end PAGE
    /begin PAGE 0x1 ECU_ACCESS_DONT_CARE XCP_READ_ACCESS_DONT_CARE XCP_WRITE_ACCESS_NOT_ALLOWED /end PAGE
    /end SEGMENT
/end IF_DATA"#,
            n + 1,
            calseg.addr_ext,
        )?;

        writeln!(writer, r#"/end MEMORY_SEGMENT"#,)?;
        //}
        Ok(())
    }
}

//-------------------------------------------------------------------------------------------------
// TYPEDEF_STRUCTURE

impl McTypeDef {
    pub fn write_typedef(&self, writer: &mut A2lWriter, level: usize) -> std::io::Result<()> {
        const EXT_FIELD_NAMES: bool = false; // Use type_name.field_name for TYPEDEF_MEASUREMENT, TYPEDEF_CHARACTERISTIC, TYPEDEF_AXIS

        let type_length = self.size;
        let type_name = self.name.as_str();

        log::debug!("A2L writer: typedef {level} {type_name} len={type_length} ");

        // Recurese or generate measurement typedefs for all TypeDefFields of this TypeDef
        for field in &self.fields {
            //
            // Field is a typedef, recursion here
            if let McValueType::TypeDef(field_type_name) = &field.dim_type.value_type {
                if let Some(typedef) = writer.registry.typedef_list.find_typedef(field_type_name.as_str()) {
                    typedef.write_typedef(writer, level + 1)?;
                } else {
                    log::error!("a2l_writer: TypeDef for {} not found", field_type_name.as_str());
                }
            } else {
                //
                // Field is a basic type

                // Generate TYPEDEF_MEASUREMENT, TYPEDEF_CHARACTERISTIC or TYPEDEF_AXIS for basic types in STRUCTURE_COMPONENT
                // @@@@ ISSUE TYPEDEF_MEASUREMENT, TYPEDEF_CHARACTERISTIC or TYPEDEF_AXIS have a unique name space
                // How to handle duplicate names ???????, this is not checked
                // Currently: Name is type_name.field_name do reduce naming conflicts, which leads to confusing names in CANape
                // TYPEDEF_MEASUREMENT, TYPEDEF_CHARACTERISTIC or TYPEDEF_AXIS do not support DISPLAY_IDENTIFIER, which would have been a solution
                let field_name = field.name.as_str();
                let field_dim_type = &field.dim_type;
                let value_type = field_dim_type.value_type;
                let tmp = format!("{type_name}.{field_name}");
                let ext_field_name = if EXT_FIELD_NAMES { tmp.as_str() } else { field_name };

                // Skip duplicate TYPEDEF_MEASUREMENT, TYPEDEF_CHARACTERISTIC, TYPEDEF_AXIS during recursion
                // @@@@ TODO Check if this is an error, because or the types are different
                // @@@@ TODO Avoid the string formating here
                if !writer.check_duplicate(ext_field_name) {
                    let conversion_name = write_conversion(writer, ext_field_name, 0, field_dim_type)?;
                    let unit = field.dim_type.get_unit();
                    match field_dim_type.get_object_type() {
                        McObjectType::Measurement => {
                            let type_str = value_type.get_type_str(); // UWORD, SWORD, ULONG, SLONG, FLOAT32_IEEE, FLOAT64_IEEE, ...
                            write!(
                                writer,
                                r#"/begin TYPEDEF_MEASUREMENT {ext_field_name} "{}" {type_str} {conversion_name} 0 0 {} {}"#, // 0 0 = resolution accuracy
                                field_dim_type.get_comment(),
                                field_dim_type.get_min().unwrap(),
                                field_dim_type.get_max().unwrap()
                            )?;
                            if !unit.is_empty() {
                                write!(writer, r#" PHYS_UNIT "{unit}""#)?;
                            }
                            write_dimensions(field_dim_type, writer)?;
                            writeln!(writer, r#" /end TYPEDEF_MEASUREMENT"#,)?;
                        }
                        McObjectType::Characteristic => {
                            let type_str = field_dim_type.value_type.get_record_layout_str();
                            let sub_type_str = get_characteristic_subtype_str(field_dim_type);
                            write!(
                                writer,
                                r#"/begin TYPEDEF_CHARACTERISTIC {ext_field_name} "{}" {sub_type_str} {type_str} 0 {conversion_name} {} {}"#,
                                field_dim_type.get_comment(),
                                field_dim_type.get_min().unwrap(),
                                field_dim_type.get_max().unwrap()
                            )?;
                            if !unit.is_empty() {
                                write!(writer, r#" PHYS_UNIT "{unit}""#)?;
                            }
                            // VAL_BLK (no x_axis and no y_axis)
                            if sub_type_str == "VAL_BLK" {
                                write_dimensions(field_dim_type, writer)?;
                            }
                            // else if it is MAP or CURVE type
                            else if sub_type_str == "MAP" || sub_type_str == "CURVE" {
                                write_axis_descr(ext_field_name, field_dim_type, writer)?;
                            }
                            writeln!(writer, r#" /end TYPEDEF_CHARACTERISTIC"#,)?;
                        }
                        McObjectType::Axis => {
                            let type_str = value_type.get_record_layout_str(); // + A_ prefix for AXIS_PTR record layouts
                            write!(
                                writer,
                                r#"/begin TYPEDEF_AXIS {ext_field_name} "{}" NO_INPUT_QUANTITY A_{type_str} 0 {conversion_name} {} {} {}"#,
                                field_dim_type.get_comment(),
                                field_dim_type.get_dim()[0],
                                field_dim_type.get_min().unwrap(),
                                field_dim_type.get_max().unwrap()
                            )?;
                            if !unit.is_empty() {
                                write!(writer, r#" PHYS_UNIT "{unit}""#)?;
                            }
                            writeln!(writer, r#" /end TYPEDEF_AXIS"#,)?;
                        }
                        McObjectType::Unspecified => {
                            // A2L needs objects type specified in typedef field leafs
                            panic!("Unspecified TYPEDEF object type, this can not be represented in A2L");
                        }
                    }
                }
            }
        }

        // Skip duplicate TYPEDEF_STRUCTURE during recursion
        // @@@@ TODO Check if this is an error, because or the types are different
        if !writer.check_duplicate(type_name) {
            // Generate structure definition which fields referencing the above field typedefs TYPEDEF_MEASUREMENT, TYPEDEF_CHARACTERISTIC or other TYPEDEF_STRUCTURE
            writeln!(writer, r#"/begin TYPEDEF_STRUCTURE {type_name} "" {type_length}"#)?;
            for field in &self.fields {
                let field_dim_type = &field.dim_type;
                // Type of the field is another struct (then its a TYPEDEF)
                if let McValueType::TypeDef(type_name) = &field_dim_type.value_type {
                    write!(writer, "\t/begin STRUCTURE_COMPONENT {} {type_name} {}", field.name.as_str(), field.offset)?;
                }
                // Type of the field is basic (then its a TYPEDEF_CHARACTERISTIC, TYPEDEF_MEASUREMENT, TYPEDEF_AXIS) with the same name as the field
                // Use extended field name type_name.field name
                else if EXT_FIELD_NAMES {
                    write!(
                        writer,
                        "\t/begin STRUCTURE_COMPONENT {} {type_name}.{} {}",
                        field.name.as_str(),
                        field.name.as_str(),
                        field.offset
                    )?;
                }
                // Use simple field name
                else {
                    write!(writer, "\t/begin STRUCTURE_COMPONENT {} {} {}", field.name.as_str(), field.name.as_str(), field.offset)?;
                }

                // Write dimensions if the field is a typedef, otherwise the TYPEDEF_MEASUREMENT, TYPEDEF_CHARACTERISTIC or TYPEDEF_AXIS has the dimensions
                if field_dim_type.is_typedef() {
                    write_dimensions(field_dim_type, writer)?;
                }
                writeln!(writer, " /end STRUCTURE_COMPONENT")?;
            }
            writeln!(writer, r#"/end TYPEDEF_STRUCTURE"#)?;
        }
        Ok(())
    }
}

impl GenerateA2l for McTypeDef {
    fn write_a2l(&self, writer: &mut A2lWriter) -> std::io::Result<()> {
        self.write_typedef(writer, 0)
    }
}

//-------------------------------------------------------------------------------------------------
// Generate MEASUREMENT or INSTANCE
// Depending on RegistryMeasurement type

impl McInstance {
    fn write_measurement(&self, writer: &mut A2lWriter) -> std::io::Result<()> {
        let name = &self.name;

        // Addressing
        let event_id = self.address.event_id();
        let (ext, addr) = self.address.get_a2l_addr(writer.registry);

        // McSupportData
        let dim_type = &self.dim_type;
        let unit = dim_type.get_unit();
        let comment = dim_type.get_comment();

        log::debug!("A2L writer: measurement {} {:?} {}:0x{:08X} event={:?}", name, dim_type.value_type, ext, addr, event_id);

        //
        // BLOB used for dynamic objects
        // With Vector specific IDL annotation
        if let McValueType::Blob(annotation) = &self.dim_type.value_type {
            let buffer_size = dim_type.get_dim()[0];
            assert!(dim_type.get_dim()[0] > 0 && dim_type.get_dim()[1] == 1, "Blob must have x_dim > 0 and y_dim == 1");

            // As BLOB string (new representation)
            write!(writer, r#"/begin BLOB {name} "{comment}" 0x{addr:X} {buffer_size} ECU_ADDRESS_EXTENSION {ext} "#)?;
            let annotation_str = annotation.as_str();
            write!(
                writer,
                r#"
{annotation_str}
/begin ANNOTATION ANNOTATION_LABEL "IsVlsd" ANNOTATION_ORIGIN "" /begin ANNOTATION_TEXT  "true" /end ANNOTATION_TEXT /end ANNOTATION
/begin ANNOTATION ANNOTATION_LABEL "MaxBufferNeeded" ANNOTATION_ORIGIN "" /begin ANNOTATION_TEXT "{buffer_size}" /end ANNOTATION_TEXT /end ANNOTATION
 "#
            )?;
            if let Some(id) = event_id {
                write_ifdata_event(id, writer)?;
            }
            writeln!(writer, r#" /end BLOB"#)?;
        }
        //
        // INSTANCE used for measurement structures
        else if let McValueType::TypeDef(type_name) = &self.dim_type.value_type {
            let instance_name = self.get_unique_name(writer.registry);
            write!(writer, r#"/begin INSTANCE {instance_name} "{comment}" {type_name} 0x{addr:X} ECU_ADDRESS_EXTENSION {ext}"#)?;
            write_dimensions(dim_type, writer)?;
            if let Some(id) = event_id {
                write_ifdata_event(id, writer)?;
            }
            writeln!(writer, r#" /end INSTANCE"#)?;
        }
        //
        // MEASUREMENT used for basic types
        else {
            let instance_name = self.get_unique_name(writer.registry);
            let instance_index = self.get_index(writer.registry);
            let min = dim_type.get_min().unwrap();
            let max = dim_type.get_max().unwrap();
            let step = dim_type.get_step();
            let type_str = self.dim_type.value_type.get_type_str(); // UWORD, SWORD, ULONG, SLONG, FLOAT32_IEEE, FLOAT64_IEEE, ...
            let conversion_name = write_conversion(writer, self.name.as_str(), instance_index, dim_type)?;
            let x_fix_axis = dim_type.get_dim()[0] > 1 && dim_type.get_x_axis_conv().is_some();
            let y_fix_axis = dim_type.get_dim()[1] > 1 && dim_type.get_y_axis_conv().is_some();
            // MEASUREMENT with multiple dimensions and fix axis is not supported !!!
            // In this special case, write a READ_ONLY CHARACTERISTIC MAP or CURVE, with fixed axis and an event
            if x_fix_axis || y_fix_axis {
                log::debug!("A2L writer: MEASUREMENT {} has multiple dimensions with fix axis", instance_name);
                let record_layout = dim_type.value_type.get_record_layout_str();
                let sub_type_str = if x_fix_axis && y_fix_axis { "MAP" } else { "CURVE" };
                write!(
                    writer,
                    r#"/begin CHARACTERISTIC {} "{}" {} 0x{:X} {} 0 {} {} {} READ_ONLY"#,
                    instance_name, comment, sub_type_str, addr, record_layout, conversion_name, min, max
                )?;
                if !unit.is_empty() {
                    write!(writer, r#" PHYS_UNIT "{}""#, unit)?;
                }
                if step.is_some() {
                    write!(writer, r#" STEP_SIZE {}"#, step.unwrap())?;
                }
                write_axis_descr(name, dim_type, writer)?;
                if ext != 0 {
                    write!(writer, " ECU_ADDRESS_EXTENSION {}", ext)?;
                }
                if let Some(id) = event_id {
                    write_ifdata_event(id, writer)?;
                }
                writeln!(writer, " /end CHARACTERISTIC")?;
            } else {
                if dim_type.get_dim()[0] > 1 && dim_type.get_x_axis_ref().is_some() {
                    log::warn!("A2L writer: MEASUREMENT {} has multiple dimensions", instance_name);
                }
                write!(
                    writer,
                    r#"/begin MEASUREMENT {instance_name} "{comment}" {type_str} {conversion_name} 0 0 {min} {max} ECU_ADDRESS 0x{addr:X}"#
                )?;
                if ext != 0 {
                    write!(writer, " ECU_ADDRESS_EXTENSION {ext}")?;
                }
                // XCP_ADDR_EXT_DYN makes it possible to write a measurement object
                // @@@@ EXPERIMENTAL - not thread safe
                if ext == McAddress::XCP_ADDR_EXT_DYN {
                    write!(writer, " READ_WRITE")?;
                }
                if !unit.is_empty() {
                    write!(writer, r#" PHYS_UNIT "{unit}""#)?;
                }
                if step.is_some() {
                    write!(writer, r#" STEP_SIZE {}"#, step.unwrap())?;
                }
                write_dimensions(dim_type, writer)?;
                if let Some(id) = event_id {
                    write_ifdata_event(id, writer)?;
                }
                writeln!(writer, r#" /end MEASUREMENT"#)?;
            }
        }

        Ok(())
    }
}

//-------------------------------------------------------------------------------------------------
// CHARACTERISTIC and AXIS_PTS
impl McInstance {
    fn write_axis(&self, writer: &mut A2lWriter) -> std::io::Result<()> {
        assert!(self.dim_type.is_axis());

        let name = &self.name;

        // Addressing
        let (ext, addr) = self.address.get_a2l_addr(writer.registry);

        // McSupportData
        let dim_type = &self.dim_type;
        let unit = dim_type.get_unit();
        let comment = dim_type.get_comment();
        let record_layout = dim_type.value_type.get_record_layout_str();
        let min = dim_type.get_min().unwrap();
        let max = dim_type.get_max().unwrap();
        let step = dim_type.mc_support_data.get_step();
        let conversion_name = write_conversion(writer, name.as_str(), 0, dim_type)?;
        write!(
            writer,
            r#"/begin AXIS_PTS {} "{}" 0x{:X} NO_INPUT_QUANTITY A_{} 0 {conversion_name} {}"#,
            name,
            comment,
            addr,
            record_layout,
            self.dim_type.get_dim()[0]
        )?;
        write!(writer, r#" {}"#, min)?;
        write!(writer, r#" {}"#, max)?;
        if step.is_some() {
            write!(writer, r#" STEP_SIZE {}"#, step.unwrap())?;
        }
        if !unit.is_empty() {
            write!(writer, r#" PHYS_UNIT "{}""#, unit)?;
        }
        if ext != 0 {
            write!(writer, " ECU_ADDRESS_EXTENSION {}", ext)?;
        }
        writeln!(writer, " /end AXIS_PTS")?;
        Ok(())
    }

    fn write_characteristic(&self, writer: &mut A2lWriter) -> std::io::Result<()> {
        let name = &self.name;

        // Addressing
        let (ext, addr) = self.address.get_a2l_addr(writer.registry);

        // McSupportData
        let dim_type = &self.dim_type;
        let unit = dim_type.get_unit();
        let comment = dim_type.get_comment();

        log::debug!("A2L writer: characteristic or instance {} {:?} {}:0x{:08X}", self.name, self.dim_type.value_type, ext, addr);

        // Special case when value type is instance: -> INSTANCE
        if let McValueType::TypeDef(type_name) = &self.dim_type.value_type {
            writeln!(writer, r#"/begin INSTANCE {} "{}" {type_name} 0x{:X} /end INSTANCE"#, name, comment, addr)?;
        }
        // All other value types: -> CHARACTERISTIC
        else {
            assert!(self.dim_type.is_calibration_object());
            let min = dim_type.get_min().unwrap();
            let max = dim_type.get_max().unwrap();
            let step = dim_type.mc_support_data.get_step();
            let sub_type_str = get_characteristic_subtype_str(&self.dim_type); // VAL_BLK, VALUE, MAP, CURVE
            let record_layout = self.dim_type.value_type.get_record_layout_str();

            // Bool: Use BOOL
            if self.dim_type.value_type == McValueType::Bool {
                write!(writer, r#"/begin CHARACTERISTIC {} "{}" VALUE 0x{:X} BOOL 0 BOOL 0 1"#, self.name, comment, addr)?;
            }
            // Other type: Use NO_COMPU_METHOD, not supported yet for CHARACTERISTIC
            else {
                let conversion_name = write_conversion(writer, name.as_str(), 0, dim_type)?;
                write!(
                    writer,
                    r#"/begin CHARACTERISTIC {} "{}" {} 0x{:X} {} 0 {conversion_name} {} {}"#,
                    name, comment, sub_type_str, addr, record_layout, min, max
                )?;
            }

            // VAL_BLK (no x_axis and no y_axis)
            if sub_type_str == "VAL_BLK" {
                write_dimensions(dim_type, writer)?;
            }
            // else it is MAP or CURVE type
            else if sub_type_str == "MAP" || sub_type_str == "CURVE" {
                write_axis_descr(name, dim_type, writer)?;
            }

            if !unit.is_empty() {
                write!(writer, r#" PHYS_UNIT "{}""#, unit)?;
            }

            if step.is_some() {
                write!(writer, r#" STEP_SIZE {}"#, step.unwrap())?;
            }
            if ext != 0 {
                write!(writer, " ECU_ADDRESS_EXTENSION {}", ext)?;
            }

            writeln!(writer, " /end CHARACTERISTIC")?;
        }
        Ok(())
    }
}

//-------------------------------------------------------------------------------------------------

pub struct A2lWriter<'a> {
    writer: &'a mut dyn Write,
    registry: &'a Registry,
    typedef_list: HashMap<String, usize>,
}

impl Write for A2lWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writer.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

impl<'a> A2lWriter<'a> {
    pub fn new(writer: &'a mut dyn Write, registry: &'a Registry) -> A2lWriter<'a> {
        A2lWriter {
            writer,
            registry,
            typedef_list: HashMap::new(), // @@@@ temporary solution to avoid duplicate typedefs
        }
    }

    fn check_duplicate(&mut self, ident: &str) -> bool {
        // @@@@ Improve
        if self.typedef_list.contains_key(ident) {
            //writeln!(self, r#"/* {} duplicate skipped */"#, ident).ok();
            return true;
        }
        self.typedef_list.insert(ident.to_string(), 0);
        false
    }

    fn write_a2l_head(&mut self, project_name: &str, module_name: &str) -> std::io::Result<()> {
        write!(
            self,
            r#"
    ASAP2_VERSION 1 71
    /begin PROJECT {project_name} ""

    /begin HEADER "Written by Vector xcp_lite A2L registry" VERSION "0.2.0" PROJECT_NO VECTOR /end HEADER

    /begin MODULE {module_name} ""
            
            /include "XCP_104.aml"
         
            /begin MOD_COMMON ""
            BYTE_ORDER MSB_LAST
            ALIGNMENT_BYTE 1
            ALIGNMENT_WORD 1
            ALIGNMENT_LONG 1
            ALIGNMENT_FLOAT16_IEEE 1
            ALIGNMENT_FLOAT32_IEEE 1
            ALIGNMENT_FLOAT64_IEEE 1
            ALIGNMENT_INT64 1
            /end MOD_COMMON
            
            /* Predefined conversion rule for bool */
            /begin COMPU_METHOD BOOL ""
                TAB_VERB "%.0" "" COMPU_TAB_REF BOOL.table
            /end COMPU_METHOD
            /begin COMPU_VTAB BOOL.table "" TAB_VERB 2
                0 "false" 1 "true"
            /end COMPU_VTAB

            /* Predefined conversion rule identity with no phys unit and zero decimal places */
            /begin COMPU_METHOD IDENTITY ""
                IDENTICAL "%.0" "" 
            /end COMPU_METHOD
    
            /* Predefined characteristic record layouts for standard types */
            /begin RECORD_LAYOUT BOOL FNC_VALUES 1 UBYTE ROW_DIR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT U8 FNC_VALUES 1 UBYTE ROW_DIR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT U16 FNC_VALUES 1 UWORD ROW_DIR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT U32 FNC_VALUES 1 ULONG ROW_DIR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT U64 FNC_VALUES 1 A_UINT64 ROW_DIR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT I8 FNC_VALUES 1 SBYTE ROW_DIR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT I16 FNC_VALUES 1 SWORD ROW_DIR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT I32 FNC_VALUES 1 SLONG ROW_DIR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT I64 FNC_VALUES 1 A_INT64 ROW_DIR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT F32 FNC_VALUES 1 FLOAT32_IEEE ROW_DIR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT F64 FNC_VALUES 1 FLOAT64_IEEE ROW_DIR DIRECT /end RECORD_LAYOUT
        
            /* Predefined axis record layouts for standard types */
            /begin RECORD_LAYOUT A_U8 AXIS_PTS_X 1 UBYTE INDEX_INCR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT A_U16 AXIS_PTS_X 1 UWORD INDEX_INCR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT A_U32 AXIS_PTS_X 1 ULONG INDEX_INCR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT A_U64 AXIS_PTS_X 1 A_UINT64 INDEX_INCR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT A_I8 AXIS_PTS_X 1 SBYTE INDEX_INCR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT A_I16 AXIS_PTS_X 1 SWORD INDEX_INCR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT A_I32 AXIS_PTS_X 1 SLONG INDEX_INCR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT A_I64 AXIS_PTS_X 1 A_INT64 INDEX_INCR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT A_F32 AXIS_PTS_X 1 FLOAT32_IEEE INDEX_INCR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT A_F64 AXIS_PTS_X 1 FLOAT64_IEEE INDEX_INCR DIRECT /end RECORD_LAYOUT

            /* Predefined measurement and characteristic typedefs for standard types */
            /begin TYPEDEF_MEASUREMENT M_BOOL "" UBYTE BOOL 0 0 0 1 /end TYPEDEF_MEASUREMENT
            /begin TYPEDEF_MEASUREMENT M_U8 "" UBYTE NO_COMPU_METHOD 0 0 0 255 /end TYPEDEF_MEASUREMENT
            /begin TYPEDEF_MEASUREMENT M_U16 "" UWORD NO_COMPU_METHOD 0 0 0 65535 /end TYPEDEF_MEASUREMENT
            /begin TYPEDEF_MEASUREMENT M_U32 "" ULONG NO_COMPU_METHOD 0 0 0 4294967295 /end TYPEDEF_MEASUREMENT
            /begin TYPEDEF_MEASUREMENT M_U64 "" A_UINT64 NO_COMPU_METHOD 0 0 0 1e12 /end TYPEDEF_MEASUREMENT
            /begin TYPEDEF_MEASUREMENT M_I8 "" SBYTE NO_COMPU_METHOD 0 0 -128 127 /end TYPEDEF_MEASUREMENT
            /begin TYPEDEF_MEASUREMENT M_I16 "" SWORD NO_COMPU_METHOD 0 0 -32768 32767 /end TYPEDEF_MEASUREMENT
            /begin TYPEDEF_MEASUREMENT M_I32 "" SLONG NO_COMPU_METHOD 0 0 -2147483648 2147483647 /end TYPEDEF_MEASUREMENT
            /begin TYPEDEF_MEASUREMENT M_I64 "" A_INT64 NO_COMPU_METHOD 0 0 -1e12 1e12 /end TYPEDEF_MEASUREMENT
            /begin TYPEDEF_MEASUREMENT M_F32 "" FLOAT32_IEEE NO_COMPU_METHOD 0 0 -1e12 1e12 /end TYPEDEF_MEASUREMENT
            /begin TYPEDEF_MEASUREMENT M_F64 "" FLOAT64_IEEE NO_COMPU_METHOD 0 0 -1e12 1e12 /end TYPEDEF_MEASUREMENT
            /begin TYPEDEF_CHARACTERISTIC C_BOOL "" VALUE U8 0 BOOL 0 1 /end TYPEDEF_CHARACTERISTIC
            /begin TYPEDEF_CHARACTERISTIC C_U8 "" VALUE U8 0 NO_COMPU_METHOD 0 255 /end TYPEDEF_CHARACTERISTIC
            /begin TYPEDEF_CHARACTERISTIC C_U16 "" VALUE U16 0 NO_COMPU_METHOD 0 65535 /end TYPEDEF_CHARACTERISTIC
            /begin TYPEDEF_CHARACTERISTIC C_U32 "" VALUE U32 0 NO_COMPU_METHOD 0 4294967295 /end TYPEDEF_CHARACTERISTIC
            /begin TYPEDEF_CHARACTERISTIC C_U64 "" VALUE U64 0 NO_COMPU_METHOD 0 1e12 /end TYPEDEF_CHARACTERISTIC
            /begin TYPEDEF_CHARACTERISTIC C_I8 "" VALUE I8 0 NO_COMPU_METHOD -128 127 /end TYPEDEF_CHARACTERISTIC
            /begin TYPEDEF_CHARACTERISTIC C_I16 "" VALUE I16 0 NO_COMPU_METHOD -32768 32767 /end TYPEDEF_CHARACTERISTIC
            /begin TYPEDEF_CHARACTERISTIC C_I32 "" VALUE I32 0 NO_COMPU_METHOD -2147483648 2147483647 /end TYPEDEF_CHARACTERISTIC
            /begin TYPEDEF_CHARACTERISTIC C_I64 "" VALUE I64 0 NO_COMPU_METHOD -1e12 1e12 /end TYPEDEF_CHARACTERISTIC
            /begin TYPEDEF_CHARACTERISTIC C_F64 "" VALUE F64 0 NO_COMPU_METHOD -1e12 1e12 /end TYPEDEF_CHARACTERISTIC
            /begin TYPEDEF_CHARACTERISTIC C_F32 "" VALUE F32 0 NO_COMPU_METHOD -1e12 1e12 /end TYPEDEF_CHARACTERISTIC
            "#,
        )
    }

    // MOD_PAR
    fn write_a2l_modpar(&mut self) -> std::io::Result<()> {
        // EPK segment
        let application = &self.registry.application;
        write!(self, "/begin MOD_PAR \"\" ")?;
        application.write_a2l(self)?;
        for s in &self.registry.cal_seg_list {
            s.write_a2l(self)?;
        }
        writeln!(self, " /end MOD_PAR")
    }

    // IF_DATA XCP
    fn write_a2l_if_data(&mut self) -> std::io::Result<()> {
        write!(
            self,
            r#"/begin IF_DATA XCP
            /begin PROTOCOL_LAYER
            0x104 1000 2000 0 0 0 0 0 252 1468 BYTE_ORDER_MSB_LAST ADDRESS_GRANULARITY_BYTE
            OPTIONAL_CMD GET_COMM_MODE_INFO
            OPTIONAL_CMD GET_ID
            OPTIONAL_CMD SET_REQUEST
            OPTIONAL_CMD SET_MTA
            OPTIONAL_CMD UPLOAD
            OPTIONAL_CMD SHORT_UPLOAD
            OPTIONAL_CMD DOWNLOAD
            OPTIONAL_CMD SHORT_DOWNLOAD
            OPTIONAL_CMD GET_CAL_PAGE
            OPTIONAL_CMD SET_CAL_PAGE
            OPTIONAL_CMD COPY_CAL_PAGE
            OPTIONAL_CMD BUILD_CHECKSUM
            OPTIONAL_CMD GET_DAQ_RESOLUTION_INFO
            OPTIONAL_CMD GET_DAQ_PROCESSOR_INFO
            OPTIONAL_CMD FREE_DAQ
            OPTIONAL_CMD ALLOC_DAQ
            OPTIONAL_CMD ALLOC_ODT
            OPTIONAL_CMD ALLOC_ODT_ENTRY
            OPTIONAL_CMD SET_DAQ_PTR
            OPTIONAL_CMD WRITE_DAQ
            OPTIONAL_CMD GET_DAQ_LIST_MODE
            OPTIONAL_CMD SET_DAQ_LIST_MODE
            OPTIONAL_CMD START_STOP_SYNCH
            OPTIONAL_CMD START_STOP_DAQ_LIST
            OPTIONAL_CMD GET_DAQ_CLOCK
            OPTIONAL_CMD WRITE_DAQ_MULTIPLE
            OPTIONAL_CMD TIME_CORRELATION_PROPERTIES
            OPTIONAL_CMD USER_CMD
            OPTIONAL_LEVEL1_CMD GET_VERSION
            /end PROTOCOL_LAYER"#
        )?;

        let event_count = self.registry.event_list.len();
        writeln!(
            self,
            "\n\n\t\t\t/begin DAQ
            DYNAMIC 0 {event_count} 0 OPTIMISATION_TYPE_DEFAULT ADDRESS_EXTENSION_FREE IDENTIFICATION_FIELD_TYPE_RELATIVE_BYTE GRANULARITY_ODT_ENTRY_SIZE_DAQ_BYTE 0xF8 OVERLOAD_INDICATION_PID
            /begin TIMESTAMP_SUPPORTED
                0x1 SIZE_DWORD UNIT_1US TIMESTAMP_FIXED
            /end TIMESTAMP_SUPPORTED
            "
        )?;

        // Eventlist
        for e in &self.registry.event_list {
            e.write_a2l(self)?;
        }

        write!(self, "\n\t\t\t/end DAQ\n")?;

        // Transport layer parameters in IF_DATA
        if let Some(xcp_tl_params) = self.registry.xcp_tl_params {
            xcp_tl_params.write_a2l(self)?;
        }

        write!(self, "\n/end IF_DATA\n\n")?;
        Ok(())
    }

    fn write_a2l_typedefs(&mut self) -> std::io::Result<()> {
        writeln!(self, "\n/* TypeDefs */")?;

        for m in &self.registry.typedef_list {
            m.write_a2l(self)?;
        }
        writeln!(self)?;
        Ok(())
    }

    fn write_a2l_measurements(&mut self) -> std::io::Result<()> {
        writeln!(self, "\n/* Measurements */")?;

        // Measurable objects with event_id
        for m in self.registry.instance_list.into_iter() {
            // If with event or explicitly a measurement object
            if m.is_measurement_object() || m.address.event_id().is_some() {
                m.write_measurement(self)?;
            }
        }

        // GROUP
        // Group root measurement
        write!(self, "/begin GROUP Measurements \"\" ROOT /begin SUB_GROUP")?;
        for e in &self.registry.event_list {
            // Ignore all but the first event instance
            if e.index > 1 {
                continue;
            }
            write!(self, " {}", e.name)?;
        }
        writeln!(self, " /end SUB_GROUP /end GROUP")?;

        // Sub group for each event with event name as group name
        for event in &self.registry.event_list {
            // Ignore all but the first event instance, and compare events by name
            if event.index > 1 {
                continue;
            }
            let event_name = &event.name;
            write!(self, "/begin GROUP {} \"\" /begin REF_MEASUREMENT", event_name)?;
            for instance in self.registry.instance_list.into_iter().filter(|i| i.is_measurement_object()) {
                if let Some(instance_event_id) = instance.address.event_id() {
                    if let Some(instance_event) = self.registry.event_list.find_event_id(instance_event_id) {
                        if *event_name == instance_event.name {
                            let name = instance.get_unique_name(self.registry);
                            write!(self, " {name}")?;
                        }
                    }
                }
            }
            writeln!(self, " /end REF_MEASUREMENT /end GROUP")?;
        }

        Ok(())
    }

    fn write_a2l_characteristics(&mut self) -> std::io::Result<()> {
        // Write all Axis
        writeln!(self, "\n/* Axis */")?;
        for a in self.registry.instance_list.into_iter() {
            // If axis
            if a.is_axis() {
                assert!(a.address.is_segment_relative());
                assert!(!a.is_measurement_object());
                a.write_axis(self)?;
            }
        }

        // Write all Characteristics
        writeln!(self, "\n/* Characteristics */")?;
        for c in self.registry.instance_list.into_iter() {
            // If not an axis, to be sure to catch all instances and assert on inconsistencies
            if !c.is_axis() {
                // This is the inverse condition of the one in write_a2l_measurements
                if !(c.is_measurement_object() || c.address.event_id().is_some()) {
                    assert!(c.address.is_segment_relative());
                    assert!(!c.is_measurement_object());
                    c.write_characteristic(self)?;
                }
            }
        }

        Ok(())
    }

    fn write_a2l_groups(&mut self) -> std::io::Result<()> {
        // Write ROOT GROUP "Characteristics" with subgroups for all calibration segments
        writeln!(self, "\n/* Characteristic and Axis Groups */")?;
        if !self.registry.cal_seg_list.len() > 0 {
            write!(self, "/begin GROUP Characteristics \"\" ROOT /begin SUB_GROUP")?;
            for s in &self.registry.cal_seg_list {
                write!(self, " {}", s.name)?;
            }
            writeln!(self, " /end SUB_GROUP /end GROUP")?;
        }

        // Write GROUPs for each calibration segment
        for s in &self.registry.cal_seg_list {
            let mut n = 0;
            for c in self.registry.instance_list.into_iter() {
                if c.get_address().is_segment_relative() && s.name == c.get_address().calseg_name().unwrap() {
                    n += 1;
                    if n == 1 {
                        write!(self, "/begin GROUP {} \"\" /begin REF_CHARACTERISTIC ", s.name)?;
                    }
                    write!(self, " {} ", c.name)?;
                }
            }
            if n > 0 {
                writeln!(self, "/end REF_CHARACTERISTIC /end GROUP")?;
            }
        }

        Ok(())
    }

    fn write_a2l_tail(&mut self) -> std::io::Result<()> {
        self.write_all("\n/end MODULE\n/end PROJECT\n".as_bytes())
    }

    pub fn write_a2l(&mut self, project_name: &str, module_name: &str) -> Result<(), std::io::Error> {
        self.write_a2l_head(project_name, module_name)?;
        self.write_a2l_modpar()?;
        if self.registry.has_xcp_params() {
            self.write_a2l_if_data()?;
        }
        self.write_a2l_typedefs()?;
        self.write_a2l_measurements()?;
        self.write_a2l_characteristics()?;
        self.write_a2l_groups()?;
        self.write_a2l_tail()?;
        Ok(())
    }
}
