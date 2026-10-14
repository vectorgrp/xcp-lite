//-----------------------------------------------------------------------------
// Module a2l
// Read, write and check A2L files

#[cfg(feature = "a2l_reader")]
pub mod a2l_reader;
pub mod a2l_writer;
#[cfg(feature = "a2l_reader")]
pub mod aml_ifdata;

use super::*;

//-------------------------------------------------------------------------------------------------
// McA2mlMode

/// How the XCP A2ML description is written into an A2L file
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum McA2mlMode {
    /// Write `/include "XCP_104.aml"`, the file XCP_104.aml must be provided next to the A2L file (default, keeps the A2L file small)
    #[default]
    Include,
    /// Embed the XCP 1.4 A2ML description (`XCP_104_AML`) as `/begin A2ML ... /end A2ML` block, the A2L file is self-contained
    Embed,
    /// No A2ML description
    None,
}

//-------------------------------------------------------------------------------------------------
// Load an A2L file with crate a2lfile

// IF_DATA XCP is always parsed with the XCP A2ML specification built into this crate (aml_ifdata.rs),
// the A2L file does not need to contain or include an A2ML description.
// An /include "XCP_104.aml" in the A2L file requires the file to exist, a2lfile fails on any missing include file.
#[cfg(feature = "a2l_reader")]
fn load_a2l_file(path: &std::path::Path, strict: bool) -> Result<(a2lfile::A2lFile, Vec<a2lfile::A2lError>), String> {
    let a2ml_spec = Some(aml_ifdata::A2MLVECTOR_TEXT.to_string());
    a2lfile::load(path, a2ml_spec, strict).map_err(|e| format!("a2lfile::load failed: {}", e))
}

impl Registry {
    //---------------------------------------------------------------------------------------------------------
    // Check A2L file

    /// Check A2L file
    /// Syntax and consistency check an A2L file
    /// IF_DATA XCP is checked with the built-in XCP A2ML specification, an A2ML description in the A2L file is not required
    #[cfg(feature = "a2l_reader")]
    pub fn check_a2l<P: AsRef<std::path::Path>>(&self, path: &P) -> Result<u32, String> {
        // Read A2L file into A2lFile
        let res = load_a2l_file(path.as_ref(), true);
        match res {
            Ok((a2l_file, log_msgs)) => {
                let mut warnings: u32 = 0;

                // Log messages
                for log_msg in log_msgs {
                    log::warn!("A2L warning: {}", log_msg);
                    warnings += 1;
                }

                // Perform an additional consistency check
                let log_msgs = a2l_file.check();
                for log_msg in log_msgs {
                    log::warn!("A2L check: {}", log_msg);
                    warnings += 1;
                }

                Ok(warnings)
            }

            Err(e) => Err(e),
        }
    }

    //---------------------------------------------------------------------------------------------------------
    // Load A2L file

    /// Load A2L file into this registry
    /// # Arguments
    /// path - path to A2L file on disk
    /// print_warnings - print warnings to log
    /// strict - enable strict mode parsing
    /// check - perform additional consistency checks and print warnings to log
    /// flatten_typedefs - flatten nested typedefs to basic type instances with mangled names
    /// IF_DATA XCP is parsed with the built-in XCP A2ML specification, an A2ML description in the A2L file is not required
    #[cfg(feature = "a2l_reader")]
    pub fn load_a2l<P: AsRef<std::path::Path>>(&mut self, path: &P, print_warnings: bool, strict: bool, check: bool, flatten_typedefs: bool) -> Result<u32, String> {
        //
        // Read A2L file from file into a2lfile::A2lFile data structure
        let res = load_a2l_file(path.as_ref(), strict);
        match res {
            Ok((a2l_file, log_msgs)) => {
                let mut warnings: u32 = 0;

                // Print all log messages
                if print_warnings {
                    log::info!("{} A2L warnings:", log_msgs.len());
                    for log_msg in log_msgs {
                        log::warn!("Warning: {}", log_msg);
                        warnings += 1;
                    }
                }

                // Perform additional consistency checks on a2lfile::A2lFile
                if check {
                    let log_msgs = a2l_file.check();
                    log::info!("{} A2L checker finding:", log_msgs.len());
                    for log_msg in log_msgs {
                        log::warn!("Check: {}", log_msg);
                        warnings += 1;
                    }
                }

                // Load (merge) a2lfile::A2lFile data structure
                self.load_a2lfile(&a2l_file)?;

                // If requested, flatten nested typedefs to basic type instances with mangled names if required
                if flatten_typedefs {
                    self.flatten_typedefs();
                }

                Ok(warnings)
            }
            Err(e) => Err(e),
        }
    }

    //---------------------------------------------------------------------------------------------------------
    // Write A2L file

    /// Write registry to an A2L file
    /// if feature a2l_reader is enabled, option to check A2L file by rereading with with crate a2lfile
    /// For testing purposed only, uses a significant amount of memory allocations
    /// # Arguments
    /// path - path to A2L file on disk
    /// check - check A2L file after writing (by reading it again with a2lfile crate)
    pub fn write_a2l<P: AsRef<std::path::Path>>(
        &self,
        path: &P,
        title_comment: &str,
        project_name: &str,
        project_description: &str,
        module_name: &str,
        project_no: &str,
        check: bool,
    ) -> Result<(), std::io::Error> {
        // Create A2L file
        log::info!("Write A2L file {:?}", path.as_ref());
        let mut a2l_file = std::fs::File::create(path)?;

        // Write UTF-8 BOM
        std::io::Write::write_all(&mut a2l_file, b"\xEF\xBB\xBF")?;

        let writer: &mut dyn std::io::Write = &mut std::io::LineWriter::new(a2l_file);
        let mut a2l_writer = a2l_writer::A2lWriter::new(writer, self);

        a2l_writer.write_a2l(title_comment, project_name, project_description, module_name, project_no)?;

        // Check A2L file just written
        #[cfg(not(feature = "a2l_reader"))]
        if check {
            log::warn!("A2L file check not available, feature a2l_reader not enabled");
        }
        #[cfg(feature = "a2l_reader")]
        if check {
            // let mut a2l_filepath: PathBuf = self.get_filename().into();
            // a2l_filepath.set_extension("a2l");
            log::info!("Check A2L file {:?}", path.as_ref());
            match self.check_a2l(path) {
                Err(e) => {
                    log::error!("A2l file check error: {}", e);
                }
                Ok(w) => {
                    if w > 0 {
                        log::warn!("A2L file check with {w} warnings !!");
                    } else {
                        log::info!("A2L file check ok");
                    }
                }
            }
        }

        Ok(())
    }
}

//-------------------------------------------------------------------------------------------------
// Tests

#[cfg(all(test, feature = "a2l_reader"))]
mod test {
    use super::*;
    use a2lfile::A2lObject;

    // Empty directory in the system temp directory, unique for this process and test
    fn test_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("xcp_registry_test_{}_{}", std::process::id(), name));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    // Registry with IF_DATA XCP, two events and a measurement with a fixed event
    fn test_registry(a2ml_mode: McA2mlMode) -> Registry {
        let mut reg = Registry::new();
        reg.application.set_info("test", "test application", 0);
        reg.set_xcp_eth_params("UDP", std::net::Ipv4Addr::new(127, 0, 0, 1), 5555);
        reg.set_a2ml_mode(a2ml_mode);
        reg.event_list.add_event(McEvent::new("event1", 0, 1, 0)).unwrap();
        reg.event_list.add_event(McEvent::new("event2", 0, 2, 0)).unwrap();
        reg.instance_list
            .add_instance(
                "counter",
                McDimType::new(McValueType::Ulong, 1, 1),
                McSupportData::new(McObjectType::Measurement),
                McAddress::new_event_abs(2, 0x100),
            )
            .unwrap();
        reg
    }

    fn write_test_a2l(dir: &std::path::Path, a2ml_mode: McA2mlMode) -> std::path::PathBuf {
        let path = dir.join("test.a2l");
        test_registry(a2ml_mode).write_a2l(&path, "test", "test", "", "test", "XCPLITE__ACSDD", false).unwrap();
        path
    }

    // The registry loaded from an A2L file has the events and the measurement with its fixed event
    fn check_loaded(path: &std::path::Path) {
        let mut reg = Registry::new();
        reg.load_a2l(&path, true, true, true, false).unwrap();
        assert_eq!(reg.event_list.find_event_id(1).unwrap().get_name(), "event1");
        assert_eq!(reg.event_list.find_event_id(2).unwrap().get_name(), "event2");
        let counter = reg.instance_list.get_instance("counter", McObjectType::Measurement, None).unwrap();
        assert_eq!(counter.get_address().get_event_id(), Some(2));
        reg.check_a2l(&path).unwrap();
    }

    // An include of a missing .aml file is an error
    // a2lfile also resolves includes relative to the current working directory, which contains XCP_104.aml when running the tests,
    // so the include is renamed to a file which does not exist
    #[test]
    fn test_load_a2l_include_missing_aml() {
        let dir = test_dir("include_missing_aml");
        let path = write_test_a2l(&dir, McA2mlMode::Include);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("/include \"XCP_104.aml\""));
        std::fs::write(&path, text.replace("/include \"XCP_104.aml\"", "/include \"XCP_104_missing.aml\"")).unwrap();
        let mut reg = Registry::new();
        assert!(reg.load_a2l(&path, true, true, true, false).is_err());
        assert!(reg.check_a2l(&path).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // /include "XCP_104.aml" with XCP_104.aml next to the A2L file
    #[test]
    fn test_load_a2l_include_aml() {
        let dir = test_dir("include_aml");
        let path = write_test_a2l(&dir, McA2mlMode::Include);
        std::fs::write(dir.join("XCP_104.aml"), crate::XCP_104_AML).unwrap();
        check_loaded(&path);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // A2ML embedded into the A2L file, or no A2ML at all
    #[test]
    fn test_load_a2l_embedded_and_no_aml() {
        let dir = test_dir("embedded_aml");
        let path = write_test_a2l(&dir, McA2mlMode::Embed);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("/begin A2ML") && !text.contains("/include"));
        check_loaded(&path);
        let path = write_test_a2l(&dir, McA2mlMode::None);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("/begin A2ML") && !text.contains("/include"));
        check_loaded(&path);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // A2L text without A2ML description, with IF_DATA XCP for the transport layer, events, a memory segment and a measurement
    fn spec_test_a2l(protocol: &str) -> String {
        let mut reg = test_registry(McA2mlMode::None);
        if protocol == "SxI" {
            reg.set_xcp_sxi_params(115200);
        } else if protocol == "TCP" {
            reg.set_xcp_eth_params("TCP", std::net::Ipv4Addr::new(127, 0, 0, 1), 5555);
        }
        reg.cal_seg_list.add_cal_seg("params", Some(0), 16).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        {
            let writer: &mut dyn std::io::Write = &mut buf;
            a2l_writer::A2lWriter::new(writer, &reg).write_a2l("test", "test", "", "test", "XCPLITE__ACSDD").unwrap();
        }
        String::from_utf8(buf).unwrap()
    }

    // All IF_DATA blocks of the module
    fn module_if_data(a2l_file: &a2lfile::A2lFile) -> Vec<&a2lfile::IfData> {
        let module = &a2l_file.project.module[0];
        let mut if_data: Vec<&a2lfile::IfData> = module.if_data.iter().collect();
        if let Some(mod_par) = &module.mod_par {
            for s in &mod_par.memory_segment {
                if_data.extend(&s.if_data);
            }
        }
        for m in &module.measurement {
            if_data.extend(&m.if_data);
        }
        for c in &module.characteristic {
            if_data.extend(&c.if_data);
        }
        for i in &module.instance {
            if_data.extend(&i.if_data);
        }
        if_data
    }

    // Structure and values of parsed IF_DATA, one item per line, tags of tagged structs and unions sorted, positions ignored
    fn generic_if_data_lines(data: &a2lfile::GenericIfData, indent: usize, lines: &mut Vec<String>) {
        use a2lfile::GenericIfData as G;
        let pad = "  ".repeat(indent);
        let tagged = |kind: &str, items: &std::collections::HashMap<String, Vec<a2lfile::GenericIfDataTaggedItem>>, lines: &mut Vec<String>| {
            lines.push(format!("{pad}{kind}"));
            let mut tags: Vec<&String> = items.keys().collect();
            tags.sort();
            for tag in tags {
                for item in &items[tag] {
                    lines.push(format!("{pad}  {tag}{}", if item.is_block { " (block)" } else { "" }));
                    generic_if_data_lines(&item.data, indent + 2, lines);
                }
            }
        };
        match data {
            G::Array(items) | G::Sequence(items) | G::Struct(_, _, items) | G::Block { items, .. } => {
                lines.push(format!("{pad}{}", if matches!(data, G::Array(_)) { "Array" } else { "Struct" }));
                for item in items {
                    generic_if_data_lines(item, indent + 1, lines);
                }
            }
            G::TaggedStruct(items) => tagged("TaggedStruct", items, lines),
            G::TaggedUnion(items) => tagged("TaggedUnion", items, lines),
            G::EnumItem(_, value) => lines.push(format!("{pad}Enum {value}")),
            G::String(_, value) => lines.push(format!("{pad}String {value:?}")),
            G::Char(_, (v, _)) => lines.push(format!("{pad}Char {v}")),
            G::Int(_, (v, _)) => lines.push(format!("{pad}Int {v}")),
            G::Long(_, (v, _)) => lines.push(format!("{pad}Long {v}")),
            G::Int64(_, (v, _)) => lines.push(format!("{pad}Int64 {v}")),
            G::UChar(_, (v, _)) => lines.push(format!("{pad}UChar {v}")),
            G::UInt(_, (v, _)) => lines.push(format!("{pad}UInt {v}")),
            G::ULong(_, (v, _)) => lines.push(format!("{pad}ULong {v}")),
            G::UInt64(_, (v, _)) => lines.push(format!("{pad}UInt64 {v}")),
            G::Float(_, v) => lines.push(format!("{pad}Float {v}")),
            G::Double(_, v) => lines.push(format!("{pad}Double {v}")),
            G::None => lines.push(format!("{pad}None")),
        }
    }

    // Content of an A2ML description file without /begin A2ML ... /end A2ML
    fn a2ml_body(a2ml: &str) -> &str {
        a2ml.trim()
            .strip_prefix("/begin A2ML")
            .and_then(|s| s.strip_suffix("/end A2ML"))
            .expect("A2ML description is not enclosed in /begin A2ML ... /end A2ML")
    }

    // Parse the A2L texts with the built-in A2ML specification and with the given A2ML specification,
    // all IF_DATA blocks must be valid with both and parsed identically
    fn compare_a2ml_specifications(a2l_texts: &[String], name: &str, a2ml: &str) {
        for text in a2l_texts {
            let (builtin, _) = a2lfile::load_from_string(text, Some(aml_ifdata::A2MLVECTOR_TEXT.to_string()), true).unwrap();
            let (file, _) = a2lfile::load_from_string(text, Some(a2ml.to_string()), true).unwrap();
            let (builtin, file) = (module_if_data(&builtin), module_if_data(&file));
            assert!(builtin.len() >= 3, "IF_DATA blocks missing in test A2L file");
            assert_eq!(builtin.len(), file.len());
            for (b, f) in builtin.iter().zip(&file) {
                assert!(
                    b.ifdata_valid && f.ifdata_valid,
                    "IF_DATA in line {} not valid, built-in {} {name} {}",
                    b.get_line(),
                    b.ifdata_valid,
                    f.ifdata_valid
                );
                let (mut b_lines, mut f_lines) = (Vec::new(), Vec::new());
                generic_if_data_lines(b.ifdata_items.as_ref().unwrap(), 0, &mut b_lines);
                generic_if_data_lines(f.ifdata_items.as_ref().unwrap(), 0, &mut f_lines);
                if let Some(i) = (0..b_lines.len().max(f_lines.len())).find(|&i| b_lines.get(i) != f_lines.get(i)) {
                    let context = |lines: &[String]| lines[i.saturating_sub(8)..(i + 4).min(lines.len())].join("\n");
                    panic!(
                        "IF_DATA in line {} parsed differently\n--- built-in (aml_ifdata.rs):\n{}\n--- {name}:\n{}",
                        b.get_line(),
                        context(&b_lines),
                        context(&f_lines)
                    );
                }
            }
        }
    }

    // Replace the text between begin and end (inclusive) in a2l
    fn replace_block(a2l: &str, begin: &str, end: &str, new: &str) -> String {
        let start = a2l.find(begin).unwrap();
        let stop = a2l[start..].find(end).unwrap() + start + end.len();
        format!("{}{}{}", &a2l[..start], new, &a2l[stop..])
    }

    // The XCP A2ML specification used for reading (aml_ifdata.rs) and the one written into A2L files (XCP_104_AML)
    // are maintained separately, both must parse IF_DATA XCP identically
    #[test]
    fn test_a2ml_specification_xcp_104() {
        // A2L files written by this crate for all transport layers
        let mut a2l_texts: Vec<String> = ["UDP", "TCP", "SxI"].iter().map(|protocol| spec_test_a2l(protocol)).collect();
        // @@@@ TODO: The writer writes FRAMING without /begin and /end, which does not match the XCP 1.4 A2ML specification
        let sxi = &mut a2l_texts[2];
        if !sxi.contains("/begin FRAMING") {
            *sxi = sxi.replace("FRAMING 0x9A 0x9B", "/begin FRAMING 0x9A 0x9B /end FRAMING");
        }

        // XCP 1.4 keywords not written by this crate: event properties, transport layer properties of Ethernet and CAN
        let udp = a2l_texts[0].replace(
            "CONSISTENCY DAQ /end EVENT",
            "CONSISTENCY DAQ /begin BUFFER_RESERVE_EVENT 10 20 /end BUFFER_RESERVE_EVENT \
             /begin CPU_LOAD_CONSUMPTION_DAQ 1.0 2.0 3.0 /begin CORE_LOAD_EP 1 0.5 /end CORE_LOAD_EP /end CPU_LOAD_CONSUMPTION_DAQ \
             /begin CPU_LOAD_CONSUMPTION_QUEUE_STIM 1.0 2.0 /begin CORE_LOAD_EP 2 0.5 /end CORE_LOAD_EP /end CPU_LOAD_CONSUMPTION_QUEUE_STIM /end EVENT",
        );
        let udp_14 = replace_block(
            &udp,
            "/begin XCP_ON_UDP_IP",
            "/end XCP_ON_UDP_IP",
            "/begin XCP_ON_UDP_IP 0x0104 5555 ADDRESS \"127.0.0.1\" MAX_BUS_LOAD 50 PACKET_ALIGNMENT PACKET_ALIGNMENT_32 \
             OPTIONAL_TL_SUBCMD GET_SLAVE_ID OPTIONAL_TL_SUBCMD GET_DAQ_CLOCK_MULTICAST /end XCP_ON_UDP_IP",
        );
        let can = replace_block(
            &udp,
            "/begin XCP_ON_UDP_IP",
            "/end XCP_ON_UDP_IP",
            "/begin XCP_ON_CAN 0x0100 CAN_ID_MASTER 0x0301 CAN_ID_SLAVE 0x01 CAN_ID_GET_DAQ_CLOCK_MULTICAST 0x0302 BAUDRATE 0x0F4240 \
             SAMPLE_POINT 0x4B SAMPLE_RATE SINGLE BTL_CYCLES 0x08 SJW 0x02 SYNC_EDGE SINGLE MEASUREMENT_SPLIT_ALLOWED \
             OPTIONAL_TL_SUBCMD GET_DAQ_ID OPTIONAL_TL_SUBCMD GET_DAQ_CLOCK_MULTICAST /end XCP_ON_CAN",
        );
        a2l_texts.extend([udp_14, can]);

        compare_a2ml_specifications(&a2l_texts, "XCP_104.aml", a2ml_body(crate::XCP_104_AML));

        // Events and fixed events are loaded from A2L files with these keywords
        let dir = test_dir("xcp_14_keywords");
        let path = dir.join("test.a2l");
        for text in &a2l_texts[3..] {
            std::fs::write(&path, text).unwrap();
            check_loaded(&path);
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // The CANape A2ML specification in the built-in A2ML specification must parse CANape IF_DATA like CANape.aml in the repository root
    #[test]
    fn test_a2ml_specification_canape() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../CANape.aml");
        let canape_aml = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        let a2l = r#"ASAP2_VERSION 1 71
/begin PROJECT test ""
/begin MODULE test ""
/begin IF_DATA CANAPE_MODULE RECORD_LAYOUT_STEPSIZE "U8" 1 2 3 4 5 6 /end IF_DATA
/begin IF_DATA CANAPE_ADDRESS_UPDATE
    EPK_ADDRESS "epk" 0 ECU_CALIBRATION_OFFSET "offset" 16
    /begin CALIBRATION_METHOD InCircuit2 "table" 0 ORIGINAL_POINTER_TABLE "flash_table" 0 FLASH_SECTION 0x8000000 0x10000 /end CALIBRATION_METHOD
    /begin MAP_SYMBOL FIRST "start" 0 LAST "end" 0 ADDRESS_MAPPING_XCP "src" "dst" /end MAP_SYMBOL
    /begin MEMORY_SEGMENT "params" FIRST "params_start" 0 LAST "params_end" 0 /end MEMORY_SEGMENT
/end IF_DATA
/begin IF_DATA CANAPE_GROUP /begin STRUCTURE_LIST "a" "b" /end STRUCTURE_LIST /end IF_DATA
/begin IF_DATA CANAPE_CAL_METHOD
    /begin CAL_PARAM_GROUP NAME "group" ADDRESS 0x1000 SIZE 16 COMMENT "" LINK_MAP "symbol" 0x1000 0 0 0 /end CAL_PARAM_GROUP
/end IF_DATA
/begin IF_DATA CNP_CREATE_INI /begin TP_BLOB 1 PARAMETER "device.ini" "$CNP_DVC_SCTN$" "name" "value" /end TP_BLOB /end IF_DATA
/begin MEASUREMENT counter "" UBYTE NO_COMPU_METHOD 0 0 0 255 ECU_ADDRESS 0x200
    /begin IF_DATA CANAPE_EXT 100 LINK_MAP "counter" 0x200 0 0 0 1 0x87 0 DISPLAY 0 0 255 VIRTUAL_CONVERSION "conv" STEPSIZE_W 1 STEPSIZE_X 2 STEPSIZE_Y 3 /end IF_DATA
/end MEASUREMENT
/end MODULE
/end PROJECT
"#;
        compare_a2ml_specifications(&[a2l.to_string()], "CANape.aml", a2ml_body(&canape_aml));
    }
}
