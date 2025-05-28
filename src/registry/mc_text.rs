// Module mc_text
// 'static str version
// Warn on heap allocations
// Types:
//  McText - used for all text in the registry
//  McIdentifier - used for all named objects in the registry
// Immutable, as_str is &'static str
// May leak memory when converting from String

use serde::Deserialize;
use serde::Serialize;

//-------------------------------------------------------------------------------------------------
// McText

#[derive(Debug, Copy, Clone, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize)]
pub struct McText(&'static str);

impl McText {
    pub fn empty() -> Self {
        McText("")
    }

    pub fn new(s: &'static str) -> Self {
        McText(s)
    }

    pub fn as_str(&self) -> &'static str {
        self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::ops::Deref for McText {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl std::fmt::Display for McText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'de> Deserialize<'de> for McText {
    fn deserialize<D>(deserializer: D) -> Result<McText, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(McText::from(s))
    }
}

// From McText to &'static str, String
//-----------------------------------

impl From<McText> for &'static str {
    fn from(name: McText) -> &'static str {
        name.0
    }
}

impl From<McText> for String {
    fn from(t: McText) -> String {
        log::debug!("From<McText> to String: &'static str '{t}' to_string()");

        t.0.to_string()
    }
}

// From String, &'static str
//--------------------------

impl From<Option<&String>> for McText {
    fn from(s: Option<&String>) -> Self {
        if let Some(s) = s {
            log::debug!("From<String> to McText: Leak String '{s}' to &'static str");
            let s = s.clone().into_boxed_str();
            let s = Box::leak(s);

            McText(s)
        } else {
            McText::empty()
        }
    }
}

impl From<&String> for McText {
    fn from(s: &String) -> Self {
        log::debug!("From<String> to McText: Leak String '{s}' to &'static str");
        let s = s.clone().into_boxed_str();
        let s = Box::leak(s);

        McText(s)
    }
}

impl From<String> for McText {
    fn from(s: String) -> Self {
        log::debug!("From<String> to McText: Leak String '{s}' to &'static str");
        let s = s.into_boxed_str();
        let s = Box::leak(s);

        McText(s)
    }
}

impl From<&'static str> for McText {
    fn from(s: &'static str) -> Self {
        McText(s)
    }
}

// Eq
//--------------------------

impl PartialEq<&str> for McText {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<McText> for &str {
    fn eq(&self, other: &McText) -> bool {
        *self == other.0
    }
}

//-------------------------------------------------------------------------------------------------
// McIdentifier

#[derive(Debug, Copy, Clone, Default, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize)]
pub struct McIdentifier(&'static str);

impl McIdentifier {
    pub fn empty() -> Self {
        McIdentifier("")
    }

    pub fn new(s: &'static str) -> Self {
        s.into()
    }

    pub fn as_str(&self) -> &'static str {
        self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::ops::Deref for McIdentifier {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl std::fmt::Display for McIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'de> Deserialize<'de> for McIdentifier {
    fn deserialize<D>(deserializer: D) -> Result<McIdentifier, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(McIdentifier::from(s))
    }
}

// From McIdentifier to &'static str, String
//-----------------------------------

impl From<McIdentifier> for &'static str {
    fn from(name: McIdentifier) -> &'static str {
        name.0
    }
}

impl From<McIdentifier> for String {
    fn from(t: McIdentifier) -> String {
        log::debug!("From<McIdentifier> to String: &'static str '{t}' to_string()");

        t.0.to_string()
    }
}

// From String, &'static str
//--------------------------

fn to_identifier(s: &mut String) {
    // Convert the string into a mutable byte slice
    let bytes = unsafe { s.as_bytes_mut() };

    for byte in bytes.iter_mut() {
        // Check if the character is not alphanumeric or an underscore
        if !byte.is_ascii_alphanumeric() && *byte != b'_' && *byte != b'.' {
            *byte = b'_'; // Replace it with an underscore
        }
    }
}

fn check_identifier(s: &str) -> bool {
    // Convert the string into a mutable byte slice
    let bytes = s.as_bytes();
    for byte in bytes.iter() {
        // Check if the character is not alphanumeric or an underscore
        if !byte.is_ascii_alphanumeric() && *byte != b'_' && *byte != b'.' {
            log::warn!("Invalid identifier: {s}");
            return false; // Invalid identifier
        }
    }
    true // Valid identifier
}

impl From<String> for McIdentifier {
    fn from(s: String) -> Self {
        log::debug!("From<String> to McIdentifier: Leak String '{s}' to &'static str");
        let mut s = s.clone();
        to_identifier(&mut s);
        let s = s.into_boxed_str();
        let s = Box::leak(s);

        McIdentifier(s)
    }
}

impl From<&'static str> for McIdentifier {
    fn from(s: &'static str) -> Self {
        assert!(check_identifier(s));

        McIdentifier(s)
    }
}

// Eq
//--------------------------

impl PartialEq<&str> for McIdentifier {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<McIdentifier> for &str {
    fn eq(&self, other: &McIdentifier) -> bool {
        *self == other.0
    }
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
// Test module

#[cfg(test)]
mod mc_text_tests {

    use crate::xcp::xcp_test::test_setup;

    use super::*;

    fn is_copy<T: Sized + Copy>() {}
    fn is_send<T: Sized + Send>() {}
    fn is_sync<T: Sized + Sync>() {}
    fn is_clone<T: Sized + Clone>() {}

    #[test]
    fn test_mc_text() {
        let _ = test_setup();

        // Check markers
        is_sync::<McText>();
        is_copy::<McText>();
        is_send::<McText>();
        is_clone::<McText>();

        let t1 = McText::new("Hello");
        assert_eq!(t1.as_str(), "Hello");
        let t2 = t1;
        assert_eq!(t2.as_str(), "Hello");

        let t: McText = "Hello".into();
        assert_eq!(t, "Hello");

        let s1 = McText::from("String".to_string());
        let s2: &'static str = s1.as_str();
        assert!(s1 == s2);
        assert!(s2 == "String");

        let s3: McText = "String".into();
        assert_eq!(s3, "String");

        let s4 = McText::from("String".to_string());
        assert_eq!(s4, "String");
        let s5: String = s4.into();
        assert_eq!(s5, "String");

        let a = McText::new("A");
        let b = McText::new("B");
        assert!(a != b);
        assert!(a == "A");
        assert!(a < b)
    }

    #[test]
    fn test_mc_identifier() {
        let _ = test_setup();

        // Check markers
        is_sync::<McIdentifier>();
        is_copy::<McIdentifier>();
        is_send::<McIdentifier>();
        is_clone::<McIdentifier>();

        let t1 = McIdentifier::new("Identifier");
        assert_eq!(t1.as_str(), "Identifier");
        let t2 = t1;
        assert_eq!(t2.as_str(), "Identifier");
        let t: McIdentifier = "Identifier".into();
        assert_eq!(t, "Identifier");

        let result = std::panic::catch_unwind(|| {
            // Creating McIdentifier with invalid characters should panic
            let _ = McIdentifier::new("Illegal Identifier");
        });
        assert!(result.is_err()); // Check if the function panicked

        let s1 = McIdentifier::from("&Legal .Identifier ".to_string());
        assert_eq!(s1.as_str(), "_Legal_.Identifier_");
    }
}
