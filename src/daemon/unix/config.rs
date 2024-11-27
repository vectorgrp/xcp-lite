use log::LevelFilter;
use std::collections::HashMap;

pub struct ProcessConfig {
    name: &'static str,
    pid_fpath: &'static str,
    cwd: &'static str,
    stdin: &'static str,
    stdout: &'static str,
    stderr: &'static str,
    log_level: LevelFilter,
    sections: Sections,
}

impl ProcessConfig {
    pub fn new(
        name: &'static str,
        pid_fpath: &'static str,
        cfg_path: &'static str,
        workdir: &'static str,
        stdin: &'static str,
        stdout: &'static str,
        stderr: &'static str,
        log_level: LevelFilter,
    ) -> Result<Self, std::io::Error> {
        let sections = Sections::from_file(cfg_path)?;

        Ok(Self {
            name: name,
            pid_fpath: pid_fpath,
            cwd: workdir,
            stdin: stdin,
            stdout: stdout,
            stderr: stderr,
            log_level: log_level,
            sections,
        })
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn pid_fpath(&self) -> &'static str {
        self.pid_fpath
    }

    pub fn workdir(&self) -> &'static str {
        self.cwd
    }

    pub fn stdin(&self) -> &'static str {
        self.stdin
    }

    pub fn stdout(&self) -> &'static str {
        self.stdout
    }

    pub fn stderr(&self) -> &'static str {
        self.stderr
    }

    pub fn sections(&self) -> &Sections {
        &self.sections
    }

    pub fn loglvl(&self) -> LevelFilter {
        self.log_level
    }
}

impl Default for ProcessConfig {
    fn default() -> Self {
        Self {
            name: "xcpd",
            cwd: "/",
            stdin: "/var/log/xcpd.log",
            stdout: "/var/log/xcpd.log",
            stderr: "/var/log/xcpd.log",
            pid_fpath: "/var/run/xcpd.pid",
            log_level: LevelFilter::Info,
            sections: Sections::default(),
        }
    }
}

pub struct Sections(HashMap<String, HashMap<String, String>>);

impl Sections {
    pub fn from_file(file: &str) -> Result<Self, std::io::Error> {
        let content = std::fs::read_to_string(file)?;
        let mut items = HashMap::new();
        let mut current_section = None;

        for line in content.lines() {
            let line = line.trim();

            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                current_section = Some(line[1..line.len() - 1].to_string());
                items.insert(current_section.clone().unwrap(), HashMap::new());
            } else if let Some(section) = &current_section {
                let mut parts = line.splitn(2, '=');
                if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                    items.get_mut(section).unwrap().insert(key.trim().to_string(), value.trim().to_string());
                }
            }
        }

        Ok(Self(items))
    }

    // Equivalent to get_value in the original implementation
    pub fn get_value(&self, section: &str, key: &str) -> Option<&String> {
        self.0.get(section).and_then(|s| s.get(key))
    }

    // Section iterator similar to the original implementation
    pub fn iterate(&self) -> SectionIter<'_> {
        SectionIter { inner: self.0.iter() }
    }
}

impl Default for Sections {
    fn default() -> Self {
        Self(HashMap::new())
    }
}

pub struct Section<'a> {
    pub name: &'a str,
    pub items: &'a HashMap<String, String>,
}

pub struct SectionItemIter<'a> {
    inner: std::collections::hash_map::Iter<'a, String, String>,
}

impl<'a> Iterator for SectionIter<'a> {
    type Item = Section<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(name, items)| Section { name: name.as_str(), items })
    }
}

pub struct SectionIter<'a> {
    inner: std::collections::hash_map::Iter<'a, String, HashMap<String, String>>,
}

impl<'a> Iterator for SectionItemIter<'a> {
    type Item = (&'a String, &'a String);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}
