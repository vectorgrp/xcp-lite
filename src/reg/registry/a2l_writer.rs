//-----------------------------------------------------------------------------
// Sub Module a2l_writer
// Export an A2L file from super::registry

use std::io::Write;

#[allow(unused_imports)]
use log::{debug, error, info, trace, warn};

use super::*;
use crate::Xcp;

trait GenerateA2l {
    fn write_a2l(&self, writer: &mut A2lWriter) -> std::io::Result<()>;
}

//-------------------------------------------------------------------------------------------------

impl GenerateA2l for RegistryXcpTransportLayer {
    fn write_a2l(&self, writer: &mut A2lWriter) -> std::io::Result<()> {
        let protocol = self.protocol_name.to_uppercase();
        let port = self.port;
        let addr = self.addr;
        trace!("write transport layer: {protocol} {addr}:{port}");
        writeln!(writer, "\n\t\t\t/begin XCP_ON_{protocol}_IP 0x104 {port} ADDRESS \"{addr}\" /end XCP_ON_{protocol}_IP")
    }
}

//-------------------------------------------------------------------------------------------------

impl GenerateA2l for RegistryEvent {
    fn write_a2l(&self, writer: &mut A2lWriter) -> std::io::Result<()> {
        let name = self.name;
        let index = self.xcp_event.get_index();
        let channel = self.xcp_event.get_channel();

        trace!("Write event {} index={}  channel={}", name, index, channel);

        // @@@@ ToDo: CANape does not accept CONSISTENCY EVENT for serialized data types
        // long name 100+1 characters
        // short name 8+1 characters
        if index > 0 {
            writeln!(
                writer,
                "/begin EVENT \"{:.98}_{}\" \"{:.6}_{}\" {} DAQ 0xFF 0 0 0 CONSISTENCY DAQ /end EVENT",
                name, index, name, index, channel
            )
        } else {
            writeln!(writer, "/begin EVENT \"{:.100}\" \"{:.8}\" {} DAQ 0xFF 0 0 0 CONSISTENCY DAQ /end EVENT", name, name, channel)
        }
    }
}

//-------------------------------------------------------------------------------------------------

impl GenerateA2l for RegistryEpk {
    fn write_a2l(&self, writer: &mut A2lWriter) -> std::io::Result<()> {
        // Add a EPK memory segment for the EPK, to include the EPK in HEX-files
        if let Some(epk) = self.epk {
            trace!("write A2lEpkMemorySegment: epk={} epk_addr=0x{:08X}", epk, self.epk_addr);
            writeln!(
                writer,
                "\n\t\t\tEPK \"{}\" ADDR_EPK 0x{:08X}\n\n\t\t\t/begin MEMORY_SEGMENT epk \"\" DATA FLASH INTERN 0x{:08X} {} -1 -1 -1 -1 -1 /end MEMORY_SEGMENT",
                epk,
                self.epk_addr,
                self.epk_addr,
                epk.len(),
            )
        } else {
            Ok(())
        }
    }
}

//-------------------------------------------------------------------------------------------------

impl GenerateA2l for RegistryCalSegList {
    fn write_a2l(&self, writer: &mut A2lWriter) -> std::io::Result<()> {
        for (n, calseg) in self.iter().enumerate() {
            trace!("write A2lMemorySegment: {}  {}:0x{:X} size={}", calseg.name, calseg.addr_ext, calseg.addr, calseg.size);
            writeln!(
                writer,
                r#" 
            /begin MEMORY_SEGMENT
                {} "" DATA FLASH INTERN 0x{:X} {} -1 -1 -1 -1 -1
                /begin IF_DATA XCP
                    /begin SEGMENT {} 2 {} 0 0
                    /begin CHECKSUM XCP_ADD_44 MAX_BLOCK_SIZE 0xFFFF EXTERNAL_FUNCTION "" /end CHECKSUM
                    /begin PAGE 0x0 ECU_ACCESS_DONT_CARE XCP_READ_ACCESS_DONT_CARE XCP_WRITE_ACCESS_DONT_CARE /end PAGE
                    /begin PAGE 0x1 ECU_ACCESS_DONT_CARE XCP_READ_ACCESS_DONT_CARE XCP_WRITE_ACCESS_NOT_ALLOWED /end PAGE
                    /end SEGMENT
                /end IF_DATA
            /end MEMORY_SEGMENT"#,
                calseg.name,
                calseg.addr,
                calseg.size,
                n + 1,
                calseg.addr_ext,
            )?;
        }
        Ok(())
    }
}

//-------------------------------------------------------------------------------------------------

impl GenerateA2l for RegistryMeasurement {
    fn write_a2l(&self, writer: &mut A2lWriter) -> std::io::Result<()> {
        let (ext, addr) = if self.addr == 0 {
            // DYN
            self.xcp_event.get_dyn_ext_addr(self.addr_offset)
        } else {
            // ABS
            Xcp::get_abs_ext_addr(self.addr)
        };

        trace!(
            "write measurement: {} {} {}:0x{:08X} event={}+{}, addr=0x{:08X}",
            self.name,
            self.datatype.get_type_str(),
            ext,
            addr,
            self.xcp_event.get_channel(),
            self.addr_offset,
            self.addr
        );

        let name = &self.name;
        let comment = self.comment;
        let unit = self.unit;
        let factor = self.factor;
        let min = self.datatype.get_min_str();
        let max = self.datatype.get_max_str();
        let offset = self.offset;
        let type_str = self.datatype.get_type_str();
        let x_dim = self.x_dim;
        let y_dim = self.y_dim;
        let event = self.xcp_event.get_channel();

        // Dynamic object as CHARACTERISTIC ASCII string with IDL annotation
        if self.datatype == RegistryDataType::Blob {
            let buffer_size = self.x_dim;
            assert!(self.x_dim > 0 && self.y_dim == 1, "Blob must have x_dim > 0 and y_dim == 1");

            // BLOB (new in CANape 22 SP3: use a BLOB instead of a CHARACTERISTIC)
            // write!(,writer,
            //     r#"/begin BLOB {name} "{comment}" 0x{addr:X} {buffer_size} ECU_ADDRESS_EXTENSION {ext} {annotation} /begin IF_DATA XCP /begin DAQ_EVENT FIXED_EVENT_LIST EVENT {event} /end DAQ_EVENT /end IF_DATA /end BLOB"#
            // )?;

            // @@@@ ToDo: Intermediate solution
            // As ASCII string (old representation)
            write!(
                writer,
                r#"/begin CHARACTERISTIC {name} "{comment}" ASCII 0x{addr:X} U8 0 NO_COMPU_METHOD 0 255 READ_ONLY NUMBER {buffer_size} ECU_ADDRESS_EXTENSION {ext} "#
            )?;

            let annotation_object_descr = self.annotation.as_ref().expect("Blob type must have annotation");
            write!(
                writer,
                r#"
{annotation_object_descr}
/begin ANNOTATION ANNOTATION_LABEL "IsVlsd" ANNOTATION_ORIGIN "" /begin ANNOTATION_TEXT  "true" /end ANNOTATION_TEXT /end ANNOTATION
/begin ANNOTATION ANNOTATION_LABEL "MaxBufferNeeded" ANNOTATION_ORIGIN "" /begin ANNOTATION_TEXT "{buffer_size}" /end ANNOTATION_TEXT /end ANNOTATION
 "#
            )?;
        } else {
            if (self.factor - 1.0).abs() > f64::EPSILON || self.offset != 0.0 || !self.unit.is_empty() {
                writeln!(writer, r#"/begin COMPU_METHOD {name}.Conv "" LINEAR "%6.3" "{unit}" COEFFS_LINEAR {factor} {offset} /end COMPU_METHOD"#)?;
                write!(
                    writer,
                    r#"/begin MEASUREMENT {name} "{comment}" {type_str} {name}.Conv 0 0 {min} {max} PHYS_UNIT "{unit}" ECU_ADDRESS 0x{addr:X} ECU_ADDRESS_EXTENSION {ext}"#
                )?;
            } else {
                write!(
                    writer,
                    r#"/begin MEASUREMENT {name} "{comment}" {type_str} NO_COMPU_METHOD 0 0 {min} {max} PHYS_UNIT "{unit}" ECU_ADDRESS 0x{addr:X} ECU_ADDRESS_EXTENSION {ext}"#
                )?;
            }

            // Measurement signals or array of signals
            if x_dim > 1 && y_dim > 1 {
                write!(writer, " MATRIX_DIM {} {}", x_dim, y_dim)?;
            } else if x_dim > 1 {
                write!(writer, " MATRIX_DIM {}", x_dim)?;
            } else if y_dim > 1 {
                write!(writer, " MATRIX_DIM {}", y_dim)?;
            }
        }

        // Fixed event
        write!(writer, " /begin IF_DATA XCP /begin DAQ_EVENT FIXED_EVENT_LIST EVENT {event} /end DAQ_EVENT /end IF_DATA")?;

        if self.datatype == RegistryDataType::Blob {
            writeln!(writer, r#" /end CHARACTERISTIC"#)?;
        } else {
            writeln!(writer, r#" /end MEASUREMENT"#)?;
        };

        Ok(())
    }
}

//-------------------------------------------------------------------------------------------------

impl GenerateA2l for RegistryCharacteristic {
    fn write_a2l(&self, writer: &mut A2lWriter) -> std::io::Result<()> {
        let characteristic_type = self.get_type_str();
        let datatype = self.datatype.get_deposit_str();

        // Calculate the address extension and address of this Characteristic
        let (a2l_ext, a2l_addr) = if let Some(calseg_name) = self.calseg_name {
            // Segment relative addressing
            let index = writer.registry.get_cal_seg_index(calseg_name).expect("unknown calseg");
            Xcp::get_calseg_ext_addr(index, self.addr_offset.try_into().expect("offset too large"))
        } else {
            // Absolute addressing
            Xcp::get_abs_ext_addr(self.addr_offset)
        };

        write!(
            writer,
            r#"
/begin CHARACTERISTIC {} "{}" {} 0x{:X} {} 0 NO_COMPU_METHOD {} {}"#,
            self.name, self.comment, characteristic_type, a2l_addr, datatype, self.min, self.max,
        )?;

        if self.x_dim > 1 || self.y_dim > 1 {
            let mut axis_par: (usize, usize, usize);
            if self.x_dim > 1 && self.y_dim > 1 {
                axis_par = (self.x_dim, self.x_dim - 1, self.x_dim);
                write!(
                    writer,
                    r#" /begin AXIS_DESCR FIX_AXIS NO_INPUT_QUANTITY NO_COMPU_METHOD  {} 0 {} FIX_AXIS_PAR_DIST 0 1 {} /end AXIS_DESCR"#,
                    axis_par.0, axis_par.1, axis_par.2
                )?;
                axis_par = (self.y_dim, self.y_dim - 1, self.y_dim);
            } else if self.x_dim > 1 {
                axis_par = (self.x_dim, self.x_dim - 1, self.x_dim);
            } else {
                axis_par = (self.y_dim, self.y_dim - 1, self.y_dim);
            }
            write!(
                writer,
                r#" /begin AXIS_DESCR FIX_AXIS NO_INPUT_QUANTITY NO_COMPU_METHOD  {} 0 {} FIX_AXIS_PAR_DIST 0 1 {} /end AXIS_DESCR"#,
                axis_par.0, axis_par.1, axis_par.2
            )?;
        }

        if !self.unit.is_empty() {
            write!(writer, r#" PHYS_UNIT "{}""#, self.unit)?;
        }

        if a2l_ext != 0 {
            write!(writer, " ECU_ADDRESS_EXTENSION {}", a2l_ext)?;
        }

        if let Some(event) = self.event {
            write!(
                writer,
                " /begin IF_DATA XCP /begin DAQ_EVENT FIXED_EVENT_LIST EVENT {} /end DAQ_EVENT /end IF_DATA",
                event.get_channel()
            )?;
        }

        write!(writer, " /end CHARACTERISTIC")?;
        Ok(())
    }
}

//-------------------------------------------------------------------------------------------------

pub struct A2lWriter<'a> {
    writer: &'a mut dyn Write,
    registry: &'a Registry,
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
        A2lWriter { writer, registry }
    }

    fn write_a2l_head(&mut self, project_name: &str, module_name: &str) -> std::io::Result<()> {
        write!(
            self,
            r#"
    ASAP2_VERSION 1 71 /* written by xcp-lite registry */
    /begin PROJECT {project_name} ""
    /begin HEADER "" VERSION "1.0" /end HEADER
    
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
            
            /begin RECORD_LAYOUT U8 FNC_VALUES 1 UBYTE ROW_DIR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT S8 FNC_VALUES 1 SBYTE ROW_DIR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT U16 FNC_VALUES 1 UWORD ROW_DIR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT S16 FNC_VALUES 1 SWORD ROW_DIR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT U32 FNC_VALUES 1 ULONG ROW_DIR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT S32 FNC_VALUES 1 SLONG ROW_DIR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT U64 FNC_VALUES 1 A_UINT64 ROW_DIR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT S64 FNC_VALUES 1 A_UINT64 ROW_DIR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT F32 FNC_VALUES 1 FLOAT32_IEEE ROW_DIR DIRECT /end RECORD_LAYOUT
            /begin RECORD_LAYOUT F64 FNC_VALUES 1 FLOAT64_IEEE ROW_DIR DIRECT /end RECORD_LAYOUT
        
/*

        /begin TYPEDEF_MEASUREMENT M_F64 "" FLOAT64_IEEE NO_COMPU_METHOD 0 0 -1e12 1e12 /end TYPEDEF_MEASUREMENT
        /begin TYPEDEF_CHARACTERISTIC C_F64 "" VALUE F64 0 NO_COMPU_METHOD -1e12 1e12 /end TYPEDEF_CHARACTERISTIC
        
        /begin TYPEDEF_MEASUREMENT M_F32 "" FLOAT32_IEEE NO_COMPU_METHOD 0 0 -1e12 1e12 /end TYPEDEF_MEASUREMENT
        /begin TYPEDEF_CHARACTERISTIC C_F32 "" VALUE F32 0 NO_COMPU_METHOD -1e12 1e12 /end TYPEDEF_CHARACTERISTIC
        
        /begin TYPEDEF_MEASUREMENT M_I64 "" A_UINT64 NO_COMPU_METHOD 0 0 -1e12 1e12 /end TYPEDEF_MEASUREMENT
        /begin TYPEDEF_CHARACTERISTIC C_I64 "" VALUE S64 0 NO_COMPU_METHOD -1e12 1e12 /end TYPEDEF_CHARACTERISTIC
        
        /begin TYPEDEF_MEASUREMENT M_I32 "" SLONG NO_COMPU_METHOD 0 0 -2147483648 2147483647 /end TYPEDEF_MEASUREMENT
        /begin TYPEDEF_CHARACTERISTIC C_I32 "" VALUE S32 0 NO_COMPU_METHOD -2147483648 2147483647 /end TYPEDEF_CHARACTERISTIC
        
        /begin TYPEDEF_MEASUREMENT M_I16 "" SWORD NO_COMPU_METHOD 0 0 -32768 32767 /end TYPEDEF_MEASUREMENT
        /begin TYPEDEF_CHARACTERISTIC C_I16 "" VALUE S16 0 NO_COMPU_METHOD -32768 32767 /end TYPEDEF_CHARACTERISTIC
        
        /begin TYPEDEF_MEASUREMENT M_I8 "" SBYTE NO_COMPU_METHOD 0 0 -128 127 /end TYPEDEF_MEASUREMENT
        /begin TYPEDEF_CHARACTERISTIC C_I8 "" VALUE S8 0 NO_COMPU_METHOD -128 127 /end TYPEDEF_CHARACTERISTIC
        
        /begin TYPEDEF_MEASUREMENT M_U8 "" UBYTE NO_COMPU_METHOD 0 0 0 255 /end TYPEDEF_MEASUREMENT
        /begin TYPEDEF_CHARACTERISTIC C_U8 "" VALUE U8 0 NO_COMPU_METHOD 0 255 /end TYPEDEF_CHARACTERISTIC
        
        /begin TYPEDEF_MEASUREMENT M_U16 "" UWORD NO_COMPU_METHOD 0 0 0 65535 /end TYPEDEF_MEASUREMENT
        /begin TYPEDEF_CHARACTERISTIC C_U16 "" VALUE U16 0 NO_COMPU_METHOD 0 65535 /end TYPEDEF_CHARACTERISTIC
        
        /begin TYPEDEF_MEASUREMENT M_U32 "" ULONG NO_COMPU_METHOD 0 0 0 4294967295 /end TYPEDEF_MEASUREMENT
        /begin TYPEDEF_CHARACTERISTIC C_U32 "" VALUE U32 0 NO_COMPU_METHOD 0 4294967295 /end TYPEDEF_CHARACTERISTIC
        
        /begin TYPEDEF_MEASUREMENT M_U64 "" A_UINT64 NO_COMPU_METHOD 0 0 0 1e12 /end TYPEDEF_MEASUREMENT
        /begin TYPEDEF_CHARACTERISTIC C_U64 "" VALUE U64 0 NO_COMPU_METHOD 0 1e12 /end TYPEDEF_CHARACTERISTIC
*/

    "#,
        )
    }

    fn write_a2l_modpar(&mut self) -> std::io::Result<()> {
        // EPK segment
        let mod_par = &self.registry.mod_par;

        // // Memory segments from calibration segments
        let memory_segments = &self.registry.cal_seg_list;

        write!(self, "\n\t\t/begin MOD_PAR \"\"")?;

        mod_par.write_a2l(self)?;
        memory_segments.write_a2l(self)?;

        writeln!(self, "\n\t\t/end MOD_PAR")
    }

    fn write_a2l_if_data(&mut self) -> std::io::Result<()> {
        write!(
            self,
            r#"
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
        for e in self.registry.event_list.iter() {
            e.write_a2l(self)?;
        }

        write!(self, "\n\t\t\t/end DAQ\n")?;

        // Transport layer parameters in IF_DATA
        if let Some(tl_params) = self.registry.tl_params {
            tl_params.write_a2l(self)?;
        }

        write!(self, "\n\t\t/end IF_DATA\n\n")?;
        Ok(())
    }

    fn write_a2l_measurements(&mut self) -> std::io::Result<()> {
        // Measurements
        for m in self.registry.measurement_list.iter() {
            m.write_a2l(self)?;
        }

        // Create a root measurement group for each event, if more than one element
        for e in self.registry.event_list.iter() {
            if e.xcp_event.get_index() > 1 {
                // Ignore all but the first event instance
                continue;
            }
            if self
                .registry
                .measurement_list
                .iter()
                .filter(|m| self.registry.event_list.get_name(m.xcp_event).unwrap() == e.name)
                .count()
                > 1
            {
                write!(self, "\n/begin GROUP {} \"\" ROOT /begin REF_MEASUREMENT", e.name)?;
                for m in self.registry.measurement_list.iter() {
                    if self.registry.event_list.get_name(m.xcp_event).unwrap() == e.name {
                        write!(self, " {}", m.name)?;
                    }
                }
                writeln!(self, " /end REF_MEASUREMENT /end GROUP")?;
            }
        }

        Ok(())
    }

    fn write_a2l_characteristics(&mut self) -> std::io::Result<()> {
        // Characteristics not in a in calibration segment
        for c in self.registry.characteristic_list.iter() {
            if c.calseg_name.is_none() {
                c.write_a2l(self)?;
            }
        }

        // Characteristics in calibration segment
        for s in self.registry.cal_seg_list.iter() {
            // Calibration segment
            for c in self.registry.characteristic_list.iter() {
                if let Some(calseg_name) = c.calseg_name {
                    if s.name == calseg_name {
                        c.write_a2l(self)?;
                    }
                }
            }
            // Characteristic group for each calibration segment
            write!(self, "\n/begin GROUP {} \"\" ROOT /begin REF_CHARACTERISTIC ", s.name)?;
            for c in self.registry.characteristic_list.iter() {
                if let Some(calseg_name) = c.calseg_name {
                    if s.name == calseg_name {
                        write!(self, " {} ", c.name.as_str())?;
                    }
                }
            }
            writeln!(self, "/end REF_CHARACTERISTIC /end GROUP\n")?;
        }

        Ok(())
    }

    fn write_a2l_tail(&mut self) -> std::io::Result<()> {
        self.write_all(
            "
    /end MODULE 
    /end PROJECT
    "
            .as_bytes(),
        )
    }

    pub fn write_a2l(&mut self, project_name: &str, module_name: &str) -> Result<(), std::io::Error> {
        self.write_a2l_head(project_name, module_name)?;
        self.write_a2l_modpar()?;
        self.write_a2l_if_data()?;
        self.write_a2l_measurements()?;
        self.write_a2l_characteristics()?;
        self.write_a2l_tail()?;

        info!("Write A2L file {},{}", project_name, module_name);

        Ok(())
    }
}
