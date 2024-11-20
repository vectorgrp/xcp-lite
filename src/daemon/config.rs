use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Result},
    path::{Path, PathBuf},
};

pub(crate) const RUNTIME_DIR: &str = "/var/run"; // For PID files
pub(crate) const DATA_DIR: &str = "/var/lib"; // For application data
pub(crate) const USER: &str = "daemon"; // Non-privileged user
pub(crate) const GROUP: &str = "daemon"; // Non-privileged group

pub struct DaemonConfig {
    runtime_dir: PathBuf,
    data_dir: PathBuf,
    user: String,
    group: String,
    sections: HashMap<String, HashMap<String, String>>,
}

// Define section iterator types
pub struct SectionIter<'a> {
    inner: std::collections::hash_map::Iter<'a, String, HashMap<String, String>>,
}

pub struct SectionItemIter<'a> {
    inner: std::collections::hash_map::Iter<'a, String, String>,
}

pub struct Section<'a> {
    pub name: &'a str,
    pub items: &'a HashMap<String, String>,
}

impl DaemonConfig {
    pub fn new<P: AsRef<Path>>(config_path: P) -> Result<Self> {
        let mut config = DaemonConfig::default();
        config.parse_file(config_path)?;
        Ok(config)
    }

    // Get the value of a key in a section. If no section is present use "default"
    pub fn get_value(&self, section: &str, key: &str) -> Option<&String> {
        self.sections.get(section).and_then(|s| s.get(key))
    }

    pub fn runtime_dir(&self) -> &PathBuf {
        &self.runtime_dir
    }

    pub fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }

    pub fn user(&self) -> &str {
        &self.user
    }

    pub fn group(&self) -> &str {
        &self.group
    }

    pub fn sections(&self) -> SectionIter<'_> {
        SectionIter { inner: self.sections.iter() }
    }

    pub fn section_items(&self, section: &str) -> Option<SectionItemIter<'_>> {
        self.sections.get(section).map(|s| SectionItemIter { inner: s.iter() })
    }

    fn parse_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);

        let mut current_section = String::from("default");

        for line in reader.lines() {
            let line = line?.trim().to_string();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Handle sections
            if line.starts_with('[') && line.ends_with(']') {
                current_section = line[1..line.len() - 1].to_string();
                self.sections.entry(current_section.clone()).or_insert_with(HashMap::new);
                continue;
            }

            // Handle key-value pairs
            if let Some(index) = line.find('=') {
                let key = line[..index].trim().to_string();
                let value = line[index + 1..].trim().to_string();

                self.sections.entry(current_section.clone()).or_insert_with(HashMap::new).insert(key, value);
            }
        }

        Ok(())
    }
}

#[cfg(unix)]
impl Default for DaemonConfig {
    fn default() -> Self {
        DaemonConfig {
            runtime_dir: PathBuf::from("/var/run"), // For PID files
            data_dir: PathBuf::from("/var/lib"),    // For application data
            user: String::from("daemon"),           // Non-privileged user
            group: String::from("daemon"),          // Non-privileged group
            sections: HashMap::new(),
        }
    }
}

impl<'a> Iterator for SectionIter<'a> {
    type Item = Section<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(name, items)| Section { name: name.as_str(), items })
    }
}

impl<'a> Iterator for SectionItemIter<'a> {
    type Item = (&'a String, &'a String);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to write dummy values to a test config file
    fn init_dummy_config_vals(test_file_path: &str) {
        std::fs::write(
            test_file_path,
            "\
    # Test config file
    key1 = value1
    [section1]
    key2 = value2
    key3 = value3
    [section2]
    key4 = value4",
        )
        .unwrap();
    }

    #[test]
    fn test_get_config_value() {
        let test_file_path = "test_config.conf";
        init_dummy_config_vals(test_file_path);

        let config = DaemonConfig::new(test_file_path).unwrap();

        assert_eq!(config.get_value("default", "key1"), Some(&String::from("value1")));
        assert_eq!(config.get_value("section1", "key2"), Some(&String::from("value2")));
        assert_eq!(config.get_value("section1", "key3"), Some(&String::from("value3")));
        assert_eq!(config.get_value("section2", "key4"), Some(&String::from("value4")));
        assert_eq!(config.get_value("default", "nonexistent"), None);
        assert_eq!(config.get_value("nonexistent_section", "key1"), None);

        std::fs::remove_file(test_file_path).unwrap();
    }


    #[test]
    fn test_section_iterator() {
        let test_file_path = "test_config.conf";
        init_dummy_config_vals(test_file_path);

        let config = DaemonConfig::new(test_file_path).unwrap();

        let mut section_count = 0;
        let mut item_count = 0;
        for section in config.sections() {
            section_count += 1;
            for _ in section.items {
                item_count += 1;
            }
        }
        assert_eq!(section_count, 3);
        assert_eq!(item_count, 4);

        std::fs::remove_file(test_file_path).unwrap();
    }
    
}
