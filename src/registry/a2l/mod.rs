//-----------------------------------------------------------------------------
// Module a2l
// Read, write and check A2L files

#[cfg(feature = "a2l_reader")]
pub mod a2l_reader;
pub mod a2l_writer;
#[cfg(feature = "a2l_reader")]
pub mod aml_ifdata;

use super::*;

impl Registry {
    //---------------------------------------------------------------------------------------------------------
    // Check A2L file

    /// Check A2L file
    /// Syntax and consistency check an A2L file
    #[cfg(feature = "a2l_reader")]
    pub fn check_a2l<P: AsRef<std::path::Path>>(&self, path: &P) -> Result<u32, String> {
        // Read A2L file into A2lFile
        let res = a2lfile::load(path, None, true);
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

            Err(e) => Err(format!("a2lfile::load failed: {:?}", e)),
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
    #[cfg(feature = "a2l_reader")]
    pub fn load_a2l<P: AsRef<std::path::Path>>(&mut self, path: &P, print_warnings: bool, strict: bool, check: bool, flatten_typedefs: bool) -> Result<u32, String> {
        //
        // Read A2L file from file into a2lfile::A2lFile data structure
        let res = a2lfile::load(path, None, strict);
        match res {
            Ok((a2l_file, log_msgs)) => {
                let mut warnings: u32 = 0;

                // Print all log messages
                if print_warnings {
                    for log_msg in log_msgs {
                        log::warn!("A2L warning: {}", log_msg);
                        warnings += 1;
                    }
                }

                // Perform additional consistency checks on a2lfile::A2lFile
                if check {
                    // let mut log_msgs = Vec::new();
                    // a2l_file.check(&mut log_msgs);
                    let log_msgs = a2l_file.check();
                    for log_msg in log_msgs {
                        log::warn!("A2L check: {}", log_msg);
                        warnings += 1;
                    }
                }

                // Load (merge) a2lfile::A2lFile data structure into this registry
                self.load_a2lfile(&a2l_file)?;

                // If requested, flatten nested typedefs to basic type instances with mangled names if required
                if flatten_typedefs {
                    self.flatten_typedefs();
                }

                Ok(warnings)
            }
            Err(e) => Err(format!("a2lfile::load failed: {:?}", e)),
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
    pub fn write_a2l<P: AsRef<std::path::Path>>(&self, path: &P, check: bool) -> Result<(), std::io::Error> {
        // Write to A2L file
        log::info!("Write A2L file {:?}", path.as_ref());
        let a2l_file = std::fs::File::create(path)?;
        let writer: &mut dyn std::io::Write = &mut std::io::LineWriter::new(a2l_file);
        let mut a2l_writer = a2l_writer::A2lWriter::new(writer, self);
        let a2l_name = self.get_app_name();
        assert!(!a2l_name.is_empty());
        a2l_writer.write_a2l(a2l_name, a2l_name)?;

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
