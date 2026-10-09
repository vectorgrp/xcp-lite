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
// a2lfile fails on any /include of a file which does not exist. An /include of a missing .aml file is ignored,
// so A2L files with /include "XCP_104.aml" can be loaded without having XCP_104.aml next to it.
#[cfg(feature = "a2l_reader")]
fn load_a2l_file(path: &std::path::Path, strict: bool) -> Result<(a2lfile::A2lFile, Vec<a2lfile::A2lError>), String> {
    let a2ml_spec = Some(aml_ifdata::A2MLVECTOR_TEXT.to_string());

    // Find /include directives which can not be resolved
    // Files in encodings other than UTF-8 or Latin-1 (UTF-16/32) yield no includes here and are left to a2lfile
    let bytes = std::fs::read(path).map_err(|e| format!("a2lfile::load failed: {}: {}", path.display(), e))?;
    let mut text = decode_a2l_text(&bytes);
    let base_dir = path.parent().unwrap_or(std::path::Path::new(""));
    let includes = find_a2l_includes(&text);
    if includes.iter().all(|inc| !is_aml_file(&inc.name) || resolve_a2l_include(&inc.name, base_dir).is_some()) {
        return a2lfile::load(path, a2ml_spec, strict).map_err(|e| format!("a2lfile::load failed: {}", e));
    }

    // Remove the /include directives of missing .aml files, keep line numbers unchanged
    // a2lfile::load_from_string resolves relative includes relative to the current working directory, not to the A2L file,
    // so all other includes are replaced by the resolved path
    for inc in includes.iter().rev() {
        if let Some(resolved) = resolve_a2l_include(&inc.name, base_dir) {
            text.replace_range(inc.name_range.clone(), &format!("\"{}\"", resolved.display()));
        } else if is_aml_file(&inc.name) {
            log::warn!("{}: included file {} not found, using the built-in XCP A2ML specification", path.display(), inc.name);
            let blank: String = text[inc.range.clone()].chars().map(|c| if c == '\n' { c } else { ' ' }).collect();
            text.replace_range(inc.range.clone(), &blank);
        }
    }
    a2lfile::load_from_string(&text, a2ml_spec, strict).map_err(|e| format!("a2lfile::load failed: {}: {}", path.display(), e))
}

// Decode A2L file content, UTF-8 (with or without BOM) or Latin-1
#[cfg(feature = "a2l_reader")]
fn decode_a2l_text(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    }
}

#[cfg(feature = "a2l_reader")]
fn is_aml_file(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".aml")
}

// An /include directive in an A2L file
#[cfg(feature = "a2l_reader")]
#[derive(Debug)]
struct A2lInclude {
    range: std::ops::Range<usize>,      // Byte range of the complete directive
    name_range: std::ops::Range<usize>, // Byte range of the filename, including quotes
    name: String,                       // Filename without quotes
}

// Find all /include directives in A2L text, skipping comments and strings (same lexical rules as a2lfile)
#[cfg(feature = "a2l_reader")]
fn find_a2l_includes(text: &str) -> Vec<A2lInclude> {
    let b = text.as_bytes();
    let len = b.len();
    let mut includes = Vec::new();
    let mut i = 0;
    while i < len {
        if b[i] == b'"' {
            i = find_a2l_string_end(b, i);
        } else if b[i..].starts_with(b"/*") {
            i = text[i + 2..].find("*/").map_or(len, |p| i + 2 + p + 2);
        } else if b[i..].starts_with(b"//") {
            i = text[i..].find('\n').map_or(len, |p| i + p);
        } else if b[i..].starts_with(b"/include") {
            let start = i;
            i += 8;
            while i < len && b[i].is_ascii_whitespace() {
                i += 1;
            }
            let name_start = i;
            if i < len && b[i] == b'"' {
                i = find_a2l_string_end(b, i);
            } else {
                while i < len && !b[i].is_ascii_whitespace() {
                    i += 1;
                }
            }
            if i > name_start {
                let name = text[name_start..i].trim_matches('"').to_string();
                includes.push(A2lInclude {
                    range: start..i,
                    name_range: name_start..i,
                    name,
                });
            }
        } else {
            i += 1;
        }
    }
    includes
}

// Position after the closing quote of the string starting at pos, quotes are escaped as \" or ""
#[cfg(feature = "a2l_reader")]
fn find_a2l_string_end(b: &[u8], pos: usize) -> usize {
    let mut i = pos + 1;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'"' if i + 1 < b.len() && b[i + 1] == b'"' => i += 2,
            b'"' => return i + 1,
            _ => i += 1,
        }
    }
    b.len()
}

// Resolve an include filename like a2lfile: absolute, relative to the directory of the including file, or relative to the current working directory
#[cfg(feature = "a2l_reader")]
fn resolve_a2l_include(name: &str, base_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let sep = std::path::MAIN_SEPARATOR.to_string();
    let name = name.replace(['\\', '/'], &sep);
    let path = std::path::Path::new(&name);
    if path.is_absolute() {
        return if path.exists() { Some(path.to_path_buf()) } else { None };
    }
    let joined = base_dir.join(path);
    if joined.exists() {
        Some(joined)
    } else if path.exists() {
        Some(path.to_path_buf())
    } else {
        None
    }
}

impl Registry {
    //---------------------------------------------------------------------------------------------------------
    // Check A2L file

    /// Check A2L file
    /// Syntax and consistency check an A2L file
    /// IF_DATA XCP is checked with the built-in XCP A2ML specification, a missing XCP_104.aml include file is ignored
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
    /// IF_DATA XCP is parsed with the built-in XCP A2ML specification, a missing XCP_104.aml include file is ignored
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

    // /include "XCP_104.aml" without XCP_104.aml next to the A2L file
    #[test]
    fn test_load_a2l_include_missing_aml() {
        let dir = test_dir("include_missing_aml");
        let path = write_test_a2l(&dir, McA2mlMode::Include);
        assert!(std::fs::read_to_string(&path).unwrap().contains("/include \"XCP_104.aml\""));
        check_loaded(&path);
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

    // Other relative includes are still resolved relative to the A2L file, when a missing XCP_104.aml include is removed
    #[test]
    fn test_load_a2l_missing_aml_other_include() {
        let dir = test_dir("other_include");
        let path = write_test_a2l(&dir, McA2mlMode::Include);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(
            dir.join("sub").join("part.a2l"),
            "/begin MEASUREMENT included \"\" UBYTE NO_COMPU_METHOD 0 0 0 255 ECU_ADDRESS 0x200 /end MEASUREMENT\n",
        )
        .unwrap();
        let text = std::fs::read_to_string(&path).unwrap().replace("/end MODULE", "/include \"sub/part.a2l\"\n/end MODULE");
        std::fs::write(&path, text).unwrap();
        check_loaded(&path);
        let mut reg = Registry::new();
        reg.load_a2l(&path, true, true, false, false).unwrap();
        assert!(reg.instance_list.get_instance("included", McObjectType::Measurement, None).is_some());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    // /include in comments and strings is not an include directive, quoted and unquoted filenames
    #[test]
    fn test_find_a2l_includes() {
        let text = "/* /include \"a.aml\" */ // /include b.aml\n\"x /include \\\"c.aml\\\" \"\"y\"\" \" /include \"d.aml\" /include\te.a2l";
        let includes = find_a2l_includes(text);
        let names: Vec<&str> = includes.iter().map(|inc| inc.name.as_str()).collect();
        assert_eq!(names, ["d.aml", "e.a2l"]);
        assert_eq!(&text[includes[0].range.clone()], "/include \"d.aml\"");
        assert_eq!(&text[includes[0].name_range.clone()], "\"d.aml\"");
        assert_eq!(&text[includes[1].range.clone()], "/include\te.a2l");
    }
}
