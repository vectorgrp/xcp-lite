//-----------------------------------------------------------------------------
// Sub Module a2l_writer
// Export an A2L file from super::registry

use std::{
    collections::hash_map::DefaultHasher,
    fs::File,
    hash::{Hash, Hasher},
    io::Write,
};

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use super::*;
use crate::Xcp;

trait GenerateA2l {
    fn to_a2l_string(&self) -> String;
}

//-------------------------------------------------------------------------------------------------

impl GenerateA2l for RegistryXcpTransportLayer {
    fn to_a2l_string(&self) -> String {
        let protocol = self.protocol_name.to_uppercase();
        let port = self.port;
        let ip = format!(
            "{}.{}.{}.{}",
            self.ip[0], self.ip[1], self.ip[2], self.ip[3]
        );
        trace!("write transport layer: {protocol} {ip}:{port}");
        format!(r#"/begin XCP_ON_{protocol}_IP 0x104 {port} ADDRESS "{ip}" /end XCP_ON_UDP_IP"#)
    }
}

//-------------------------------------------------------------------------------------------------

impl GenerateA2l for XcpEvent {
    fn to_a2l_string(&self) -> String {
        let indexed_name = self.get_indexed_name();
        trace!(
            "write Event {} / {}  num={}",
            self.get_name(),
            self.get_indexed_name(),
            self.get_num()
        );
        // long name 100+1 characters
        // short name 8+1 characters
        format!(
            r#"/begin EVENT "{:.100}" "{:.8}" {} DAQ 0xFF 0 0 0 CONSISTENCY EVENT /end EVENT"#,
            indexed_name,
            indexed_name,
            self.get_num()
        )
    }
}

//-------------------------------------------------------------------------------------------------

impl GenerateA2l for RegistryEpk {
    fn to_a2l_string(&self) -> String {
        // Add a EPK memory segment for the EPK, to include the EPK in HEX-files
        if let Some(epk) = self.epk {
            trace!(
                "write A2lEpkMemorySegment: epk={} epk_addr=0x{:08X}",
                epk,
                self.epk_addr
            );
            format!(
                r#"
        EPK "{}"
        ADDR_EPK 0x{:08X}
        /begin MEMORY_SEGMENT
            epk "" DATA FLASH INTERN 0x{:08X} {} -1 -1 -1 -1 -1
        /end MEMORY_SEGMENT"#,
                epk,
                self.epk_addr,
                self.epk_addr,
                epk.len(),
            )
        } else {
            "".to_string()
        }
    }
}

//-------------------------------------------------------------------------------------------------

impl GenerateA2l for RegistryCalSegList {
    fn to_a2l_string(&self) -> String {
        // Add all memory segments from the calibration segment list
        let mut s = String::new();
        let mut n: u32 = 0;
        for calseg in self.iter() {
            n += 1;
            trace!(
                "write A2lMemorySegment: {}  {}:0x{:X} size={}",
                calseg.name,
                calseg.addr_ext,
                calseg.addr,
                calseg.size
            );
            s = s
                + &format!(
                    r#" 
        /begin MEMORY_SEGMENT
            {} "" DATA FLASH INTERN 0x{:X} {} -1 -1 -1 -1 -1
            /begin IF_DATA XCP
                /begin SEGMENT /* index: */ {n} /* pages: */ 2 /* ext: */ {} 0 0
                /begin CHECKSUM XCP_ADD_44 MAX_BLOCK_SIZE 0xFFFF EXTERNAL_FUNCTION "" /end CHECKSUM
                /begin PAGE 0x0 ECU_ACCESS_DONT_CARE XCP_READ_ACCESS_DONT_CARE XCP_WRITE_ACCESS_DONT_CARE /end PAGE
                /begin PAGE 0x1 ECU_ACCESS_DONT_CARE XCP_READ_ACCESS_DONT_CARE XCP_WRITE_ACCESS_NOT_ALLOWED /end PAGE
                /end SEGMENT
            /end IF_DATA
        /end MEMORY_SEGMENT"#,
                    calseg.name, calseg.addr, calseg.size, calseg.addr_ext,
                );
        }
        s
    }
}

//-------------------------------------------------------------------------------------------------

impl GenerateA2l for RegistryMeasurement {
    fn to_a2l_string(&self) -> String {
        let (ext, addr) = self.event.get_daq_ext_addr(self.event_offset);

        trace!(
            "write measurement: {} {} {}:0x{:08X} event={},{}",
            self.name,
            self.datatype.get_type_str(),
            ext,
            addr,
            self.event.get_num(),
            self.event_offset
        );

        let name = &self.name;
        let comment = self.comment;
        let unit = self.unit;
        let factor = self.factor;
        let max = self.datatype.get_max();
        let offset = self.offset;
        let type_str = self.datatype.get_type_str();
        let dim = self.dim;
        let min = self.datatype.get_min();
        let event = self.event.get_num();

        //TODO: Maybe rework strings and add VALID and BUFFER constants
        // Dynamic object as CHARACTERISTIC ASCII string with IDL annotation
        if self.datatype == RegistryDataType::Blob {
            let buffer_size = self.dim;

            let annotation = self.annotation.as_ref().unwrap();

            let annotation = format!(
                r#"
                {annotation}
                /begin ANNOTATION ANNOTATION_LABEL "IsVlsd" ANNOTATION_ORIGIN "" /begin ANNOTATION_TEXT  "true" /end ANNOTATION_TEXT /end ANNOTATION
                /begin ANNOTATION ANNOTATION_LABEL "MaxBufferNeeded" ANNOTATION_ORIGIN "" /begin ANNOTATION_TEXT "{buffer_size}" /end ANNOTATION_TEXT /end ANNOTATION
            "#
            );

            trace!("write measurement dynamic object description: {annotation}");
            // As BLOB (new in CANape 22 SP3)
            // format!(
            //     r#"/begin BLOB {name} "{comment}" 0x{addr:X} {buffer_size} ECU_ADDRESS_EXTENSION {ext} {annotation} /begin IF_DATA XCP /begin DAQ_EVENT FIXED_EVENT_LIST EVENT {event} /end DAQ_EVENT /end IF_DATA /end BLOB"#
            // )
            // As ASCII string (old representation)
            format!(
                r#"/begin CHARACTERISTIC {name} "{comment}" ASCII 0x{addr:X} U8 0 NO_COMPU_METHOD 0 255 READ_ONLY NUMBER {buffer_size} ECU_ADDRESS_EXTENSION {ext} {annotation} /begin IF_DATA XCP /begin DAQ_EVENT FIXED_EVENT_LIST EVENT {event} /end DAQ_EVENT /end IF_DATA /end CHARACTERISTIC"#
            )
        } else {
            // Measurement signals or array of signals
            let matrix_dim = if dim > 1 {
                format!("MATRIX_DIM {} ", dim)
            } else {
                "".to_string()
            };
            if self.factor != 1.0 || self.offset != 0.0 || !self.unit.is_empty() {
                format!(
                    r#"/begin COMPU_METHOD {name}.Conv "" LINEAR "%6.3" "{unit}" COEFFS_LINEAR {factor} {offset} /end COMPU_METHOD
/begin MEASUREMENT {name} "{comment}" {type_str} {name}.Conv 0 0 {min} {max} PHYS_UNIT "{unit}" ECU_ADDRESS 0x{addr:X} ECU_ADDRESS_EXTENSION {ext} {matrix_dim}/begin IF_DATA XCP /begin DAQ_EVENT FIXED_EVENT_LIST EVENT {event} /end DAQ_EVENT /end IF_DATA /end MEASUREMENT"#
                )
            } else {
                format!(
                    r#"/begin MEASUREMENT {name} "{comment}" {type_str} NO_COMPU_METHOD 0 0 {min} {max} PHYS_UNIT "{unit}" ECU_ADDRESS 0x{addr:X} ECU_ADDRESS_EXTENSION {ext} {matrix_dim}/begin IF_DATA XCP /begin DAQ_EVENT FIXED_EVENT_LIST EVENT {event} /end DAQ_EVENT /end IF_DATA /end MEASUREMENT"#
                )
            }
        }
    }
}

//-------------------------------------------------------------------------------------------------

impl GenerateA2l for RegistryCharacteristic {
    fn to_a2l_string(&self) -> String {
        let characteristic_type = self.characteristic_type();

        let datatype = RegistryDataType::from_rust_type(self.datatype).get_deposit_str();

        let (a2l_ext, a2l_addr) = Xcp::get_calseg_ext_addr(self.calseg_name, self.offset);

        let mut result = format!(
            r#"/begin CHARACTERISTIC {} "{}" {} 0x{:X} {} 0 NO_COMPU_METHOD {} {}"#,
            self.name, self.comment, characteristic_type, a2l_addr, datatype, self.min, self.max,
        );

        if self.x_dim > 1 || self.y_dim > 1 {
            let mut axis_par: (usize, usize, usize);
            if self.x_dim > 1 && self.y_dim > 1 {
                axis_par = (self.x_dim, self.x_dim - 1, self.x_dim);
                result += &format!(
                    r#" /begin AXIS_DESCR FIX_AXIS NO_INPUT_QUANTITY NO_COMPU_METHOD  {} 0 {} FIX_AXIS_PAR_DIST 0 1 {} /end AXIS_DESCR"#,
                    axis_par.0, axis_par.1, axis_par.2
                );
                axis_par = (self.y_dim, self.y_dim - 1, self.y_dim);
            } else if self.x_dim > 1 {
                axis_par = (self.x_dim, self.x_dim - 1, self.x_dim);
            } else {
                axis_par = (self.y_dim, self.y_dim - 1, self.y_dim);
            }
            result += &format!(
                r#" /begin AXIS_DESCR FIX_AXIS NO_INPUT_QUANTITY NO_COMPU_METHOD  {} 0 {} FIX_AXIS_PAR_DIST 0 1 {} /end AXIS_DESCR"#,
                axis_par.0, axis_par.1, axis_par.2
            );
        }

        if !self.unit.is_empty() {
            result += &format!(r#" PHYS_UNIT "{}""#, self.unit);
        }

        if a2l_ext != 0 {
            result += &format!(r#" ECU_ADDRESS_EXTENSION {}"#, a2l_ext);
        }

        result += r#" /end CHARACTERISTIC"#;

        result
    }
}

//-------------------------------------------------------------------------------------------------

#[derive(Debug)]
pub struct A2lWriter {}

impl A2lWriter {
    pub fn new() -> A2lWriter {
        A2lWriter {}
    }

    // Calculate hash of A2L string
    fn calc_hash(&self, a2l_string: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        a2l_string.hash(&mut hasher);
        let a2l_hash: u64 = hasher.finish();
        debug!("Current A2L hash = {}", a2l_hash);
        a2l_hash
    }

    // Read hash from file <a2l_name>.a2h
    fn read_hash(&self, a2l_name: &str) -> u64 {
        match std::fs::read_to_string(format!("{}.a2h", a2l_name)) {
            Ok(s) => s.parse().expect("parse a2h file failed"),
            Err(_) => 0,
        }
    }

    // Write hash to file <a2l_name>.a2h
    fn write_hash(&self, a2l_name: &str, a2l_hash: u64) {
        let a2l_hash_path = format!("{}.a2h", a2l_name);
        let mut a2l_hash_file = File::create(a2l_hash_path).expect("create hash file failed");
        write!(a2l_hash_file, "{}", a2l_hash).expect("write file failed");
    }

    pub fn write_a2l(&self, registry: &Registry) -> Result<bool, &'static str> {
        if let Some(name) = &registry.name {
            // Create A2L as string
            let a2l_string = self.get_string(registry);

            // Write A2L file on disk only if it is different to previous one
            let a2l_hash = self.calc_hash(&a2l_string);
            let a2l_hash_previous = self.read_hash(name);
            if a2l_hash_previous != a2l_hash {
                self.write_hash(name, a2l_hash);
                let a2l_path = format!("{}.a2l", name);
                let mut a2l_file = File::create(&a2l_path).expect("create a2l file failed");
                write!(a2l_file, "{}", a2l_string).expect("write file failed");
                info!("Write A2L file {}", a2l_path);
                Ok(true)
            } else {
                info!("A2L file is up to date");
                Ok(false)
            }
        } else {
            let e = "No A2L file";
            Err(e)
        }
    }

    // Format A2L as a String
    fn get_string(&self, registry: &Registry) -> String {
        // Name
        let a2l_name = registry.name.unwrap();

        // Transport layer parameters in IF_DATA
        let transport_layer = if let Some(tl_params) = registry.tl_params {
            tl_params.to_a2l_string()
        } else {
            "".to_string()
        };

        // Events
        let mut v = Vec::new();
        for e in registry.event_list.iter() {
            v.push(e.to_a2l_string());
        }
        let event_list = v.join("\n");
        let event_count = registry.event_list.len();

        // Measurements
        let mut v = Vec::new();
        for m in registry.measurement_list.iter() {
            v.push(m.to_a2l_string());
        }
        let measurements = v.join("\n");

        // Measurement groups
        let mut v = Vec::new();
        for e in registry.event_list.iter() {
            v.push(format!(
                r#"/begin GROUP {} "" /begin REF_MEASUREMENT"#,
                e.get_indexed_name()
            ));

            for m in registry.measurement_list.iter() {
                if m.event == *e {
                    v.push(m.name.clone());
                }
            }
            v.push((r#"/end REF_MEASUREMENT /end GROUP"#).to_string());
        }
        let measurement_groups = v.join("\n");

        // EPK segment
        let mod_par = &registry.mod_par.to_a2l_string();

        // Memory segments from calibration segments
        let memory_segments = &registry.cal_seg_list.to_a2l_string();

        // Parameter groups defined by calibration segments
        let mut v = Vec::new();
        for s in registry.cal_seg_list.iter() {
            for c in registry.characteristic_list.iter() {
                if s.name == c.calseg_name() {
                    v.push(c.to_a2l_string());
                }
            }
            v.push(format!(
                r#"/begin GROUP {} "" /begin REF_CHARACTERISTIC"#,
                s.name
            ));
            for c in registry.characteristic_list.iter() {
                if s.name == c.calseg_name() {
                    v.push(c.name().to_string());
                }
            }
            v.push((r#"/end REF_CHARACTERISTIC /end GROUP"#).to_string());
        }
        let characteristics = v.join("\n");

        // A2L file
        let a2l_string = format!(
            r#"
    ASAP2_VERSION 1 71
    /begin PROJECT {a2l_name} ""
        /begin HEADER "" VERSION "1.0" /end HEADER
        /begin MODULE {a2l_name} ""
    
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
    
        /begin RECORD_LAYOUT F64 FNC_VALUES 1 FLOAT64_IEEE ROW_DIR DIRECT /end RECORD_LAYOUT
        /begin TYPEDEF_MEASUREMENT M_F64 "" FLOAT64_IEEE NO_COMPU_METHOD 0 0 -1e12 1e12 /end TYPEDEF_MEASUREMENT
        /begin TYPEDEF_CHARACTERISTIC C_F64 "" VALUE F64 0 NO_COMPU_METHOD -1e12 1e12 /end TYPEDEF_CHARACTERISTIC
        
        /begin RECORD_LAYOUT F32 FNC_VALUES 1 FLOAT32_IEEE ROW_DIR DIRECT /end RECORD_LAYOUT
        /begin TYPEDEF_MEASUREMENT M_F32 "" FLOAT32_IEEE NO_COMPU_METHOD 0 0 -1e12 1e12 /end TYPEDEF_MEASUREMENT
        /begin TYPEDEF_CHARACTERISTIC C_F32 "" VALUE F32 0 NO_COMPU_METHOD -1e12 1e12 /end TYPEDEF_CHARACTERISTIC
        
        /begin RECORD_LAYOUT S64 FNC_VALUES 1 A_UINT64 ROW_DIR DIRECT /end RECORD_LAYOUT
        /begin TYPEDEF_MEASUREMENT M_I64 "" A_UINT64 NO_COMPU_METHOD 0 0 -1e12 1e12 /end TYPEDEF_MEASUREMENT
        /begin TYPEDEF_CHARACTERISTIC C_I64 "" VALUE S64 0 NO_COMPU_METHOD -1e12 1e12 /end TYPEDEF_CHARACTERISTIC
        
        /begin RECORD_LAYOUT S32 FNC_VALUES 1 SLONG ROW_DIR DIRECT /end RECORD_LAYOUT
        /begin TYPEDEF_MEASUREMENT M_I32 "" SLONG NO_COMPU_METHOD 0 0 -2147483648 2147483647 /end TYPEDEF_MEASUREMENT
        /begin TYPEDEF_CHARACTERISTIC C_I32 "" VALUE S32 0 NO_COMPU_METHOD -2147483648 2147483647 /end TYPEDEF_CHARACTERISTIC
        
        /begin RECORD_LAYOUT S16 FNC_VALUES 1 SWORD ROW_DIR DIRECT /end RECORD_LAYOUT
        /begin TYPEDEF_MEASUREMENT M_I16 "" SWORD NO_COMPU_METHOD 0 0 -32768 32767 /end TYPEDEF_MEASUREMENT
        /begin TYPEDEF_CHARACTERISTIC C_I16 "" VALUE S16 0 NO_COMPU_METHOD -32768 32767 /end TYPEDEF_CHARACTERISTIC
        
        /begin RECORD_LAYOUT S8 FNC_VALUES 1 SBYTE ROW_DIR DIRECT /end RECORD_LAYOUT
        /begin TYPEDEF_MEASUREMENT M_I8 "" SBYTE NO_COMPU_METHOD 0 0 -128 127 /end TYPEDEF_MEASUREMENT
        /begin TYPEDEF_CHARACTERISTIC C_I8 "" VALUE S8 0 NO_COMPU_METHOD -128 127 /end TYPEDEF_CHARACTERISTIC
        
        /begin RECORD_LAYOUT U8 FNC_VALUES 1 UBYTE ROW_DIR DIRECT /end RECORD_LAYOUT
        /begin TYPEDEF_MEASUREMENT M_U8 "" UBYTE NO_COMPU_METHOD 0 0 0 255 /end TYPEDEF_MEASUREMENT
        /begin TYPEDEF_CHARACTERISTIC C_U8 "" VALUE U8 0 NO_COMPU_METHOD 0 255 /end TYPEDEF_CHARACTERISTIC
        
        /begin RECORD_LAYOUT U16 FNC_VALUES 1 UWORD ROW_DIR DIRECT /end RECORD_LAYOUT
        /begin TYPEDEF_MEASUREMENT M_U16 "" UWORD NO_COMPU_METHOD 0 0 0 65535 /end TYPEDEF_MEASUREMENT
        /begin TYPEDEF_CHARACTERISTIC C_U16 "" VALUE U16 0 NO_COMPU_METHOD 0 65535 /end TYPEDEF_CHARACTERISTIC
        
        /begin RECORD_LAYOUT U32 FNC_VALUES 1 ULONG ROW_DIR DIRECT /end RECORD_LAYOUT
        /begin TYPEDEF_MEASUREMENT M_U32 "" ULONG NO_COMPU_METHOD 0 0 0 4294967295 /end TYPEDEF_MEASUREMENT
        /begin TYPEDEF_CHARACTERISTIC C_U32 "" VALUE U32 0 NO_COMPU_METHOD 0 4294967295 /end TYPEDEF_CHARACTERISTIC
        
        /begin RECORD_LAYOUT U64 FNC_VALUES 1 A_UINT64 ROW_DIR DIRECT /end RECORD_LAYOUT
        /begin TYPEDEF_MEASUREMENT M_U64 "" A_UINT64 NO_COMPU_METHOD 0 0 0 1e12 /end TYPEDEF_MEASUREMENT
        /begin TYPEDEF_CHARACTERISTIC C_U64 "" VALUE U64 0 NO_COMPU_METHOD 0 1e12 /end TYPEDEF_CHARACTERISTIC
    
    /begin MOD_PAR ""
    {mod_par}
    {memory_segments}
    /end MOD_PAR
    
        /begin IF_DATA XCP
            /begin PROTOCOL_LAYER
            0x104 1000 2000 0 0 0 0 0 252 1468 BYTE_ORDER_MSB_LAST ADDRESS_GRANULARITY_BYTE
            OPTIONAL_CMD GET_COMM_MODE_INFO
            OPTIONAL_CMD GET_ID
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
            /end PROTOCOL_LAYER
            /begin DAQ
            DYNAMIC 0 {event_count} 0 OPTIMISATION_TYPE_DEFAULT ADDRESS_EXTENSION_FREE IDENTIFICATION_FIELD_TYPE_RELATIVE_BYTE GRANULARITY_ODT_ENTRY_SIZE_DAQ_BYTE 0xF8 OVERLOAD_INDICATION_PID
            /begin TIMESTAMP_SUPPORTED
                0x1 SIZE_DWORD UNIT_1US TIMESTAMP_FIXED
            /end TIMESTAMP_SUPPORTED

{event_list}

            /end DAQ

{transport_layer}

        /end IF_DATA

{characteristics}

{measurements}
{measurement_groups}

        /end MODULE
    /end PROJECT
    "#
        );

        trace!("--------------------------------------------------");

        a2l_string
    }
}

//-------------------------------------------------------------------------------------------------
// Dynamic object example
/*

/begin CHARACTERISTIC NewString ""
      ASCII 0x80020080 __UByte_Value 0 NO_COMPU_METHOD 0 255
      /begin ANNOTATION
        ANNOTATION_LABEL "IsVlsd"
        ANNOTATION_ORIGIN ""
        /begin ANNOTATION_TEXT
          "true"
        /end ANNOTATION_TEXT
      /end ANNOTATION
      /begin ANNOTATION
        ANNOTATION_LABEL "MaxBufferNeeded"
        ANNOTATION_ORIGIN ""
        /begin ANNOTATION_TEXT
          "256"
        /end ANNOTATION_TEXT
      /end ANNOTATION
      /begin ANNOTATION
        ANNOTATION_LABEL "ObjectDescription"
        ANNOTATION_ORIGIN "application/dds"
        /begin ANNOTATION_TEXT

          "module Vector {"
          "  struct SphericalPoint {"
          "    float AzimuthAngle;"
          "    float RadialDistance;"
          "    float PolarAngle;"
          "    int32 Intensity;"
          "  };"
          "  struct SphericalPointCloud {"
          "    uint32 TimestampFirst;"
          "    uint32 TimestampLast;"
          "    sequence<SphericalPoint> Points;"
          "}; }"

        /end ANNOTATION_TEXT
      /end ANNOTATION
      READ_ONLY
      ECU_ADDRESS_EXTENSION 0x0
      NUMBER 0
      /begin IF_DATA XCP
        /begin DAQ_EVENT FIXED_EVENT_LIST
          EVENT 0x00
        /end DAQ_EVENT
      /end IF_DATA
    /end CHARACTERISTIC



    /begin CHARACTERISTIC TestBlob "comment"
      VAL_BLK 0x0 U8 0 NO_COMPU_METHOD 0 255
      /begin ANNOTATION
        ANNOTATION_LABEL "IsVlsd"
        ANNOTATION_ORIGIN ""
        /begin ANNOTATION_TEXT
          "true"
        /end ANNOTATION_TEXT
      /end ANNOTATION
      /begin ANNOTATION
        ANNOTATION_LABEL "MaxBufferNeeded"
        ANNOTATION_ORIGIN ""
        /begin ANNOTATION_TEXT
          "<DynamicObject> "
          "<RootType>Vector::SphericalPointCloud</RootType>"
          "</DynamicObject>"
          ""
        /end ANNOTATION_TEXT
      /end ANNOTATION
      /begin ANNOTATION
        ANNOTATION_LABEL "ObjectDescription"
        ANNOTATION_ORIGIN "application/dds"
        /begin ANNOTATION_TEXT
          ""
          ""
          "module Vector {"
          "  struct SphericalPoint {"
          "    float AzimuthAngle;"
          "    float RadialDistance;"
          "    float PolarAngle;"
          "    int32 Intensity;"
          "  };"
          "  struct SphericalPointCloud {"
          "    uint32 TimestampFirst;"
          "    uint32 TimestampLast;"
          "    sequence<SphericalPoint> Points;"
          "}; }"
          ""
        /end ANNOTATION_TEXT
      /end ANNOTATION
      READ_ONLY
      DISPLAY_IDENTIFIER displayname
      ECU_ADDRESS_EXTENSION 0x1
      MATRIX_DIM 256
      /begin IF_DATA XCP
        /begin DAQ_EVENT FIXED_EVENT_LIST
          EVENT 0x00
        /end DAQ_EVENT
      /end IF_DATA
    /end CHARACTERISTIC




    */
