// Module mc_support
// McObjectType - Copy,Clone
// McObjectQualifier - Copy,Clone
// McSupportData - Clone with warning

use serde::{Deserialize, Serialize};

use super::McIdentifier;
use super::McText;

//----------------------------------------------------------------------------------------------
// McObjectType

/// Object type Measurement, Characteristic or Axis instances or typedefs
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum McObjectType {
    #[default]
    Unspecified = 0,
    Measurement = 1,
    Characteristic = 2,
    Axis = 3,
}

impl McObjectType {
    // Measurement object
    // Could be a explicit measurement object or a typedef
    pub fn is_measurement_object(self) -> bool {
        self == McObjectType::Measurement
    }

    // Calibration object with calibration parameter semantic
    // Could be a explicit calibration or axis object or a typedef instance with calibration semantics
    // Is constant in target software, so it is never modified by the target ECU
    pub fn is_calibration_object(self) -> bool {
        self == McObjectType::Characteristic || self == McObjectType::Axis || self == McObjectType::Unspecified
    }

    // Sub attributes of calibration_object which is axis, characteristic or typedef
    pub fn is_axis(self) -> bool {
        self == McObjectType::Axis
    }

    // Characteristic object
    pub fn is_characteristic(self) -> bool {
        self == McObjectType::Characteristic
    }
}

impl std::fmt::Display for McObjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)?;
        Ok(())
    }
}

//----------------------------------------------------------------------------------------------
// McObjectQualifier

// Object qualifier
// Independent from McObjectType (Measurement, Characteristic or Axis), an object may be volatile or constant
// This is often associated to the terms characteristic and measurement
// To avoid confusion, the terms volatile and non-volatile are used
//
// Volatile means, that objects may be continuously modified in memory by the target
//  * Measurement objects are typically volatile, but characteristics and axis objects may be volatile as well
//
// Constant means objects are never modified by the target ECU
//  * Constant object needs interior mutable to be adjustable by a calibration tool
//  * How to achieve interior mutability for constant objects is called the calibration concept
//  * Constant objects are typically characteristics or axis objects
//

/// Object qualifier for Measurement, Characteristic or Axis object type instances or typedefs
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum McObjectQualifier {
    #[default]
    Unspecified = 0,
    Volatile = 1,      // continuously modified by the target
    ReadOnly = 2,      // no async write possible, assumed volatile
    NoAsyncAccess = 4, // assumed volatile
}

impl McObjectQualifier {
    // Assumed to be continuously modified by the target
    pub fn is_volatile(self) -> bool {
        self != McObjectQualifier::Unspecified
    }
    pub fn is_unspecified(&self) -> bool {
        // used for serde, need &self
        *self == McObjectQualifier::Unspecified
    }
}

impl std::fmt::Display for McObjectQualifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)?;
        Ok(())
    }
}

//----------------------------------------------------------------------------------------------
// McSupportData

/// Metadata for measurement and calibration (characteristic or axis)
/// Instances of type Characteristic may have references to Axis
/// Not copy to inhibit that users unnecessary copy the data
#[derive(Debug, Serialize, Deserialize)]
pub struct McSupportData {
    pub object_type: McObjectType, // Measurement, Characteristic or Axis

    #[serde(skip_serializing_if = "McObjectQualifier::is_unspecified")]
    #[serde(default)]
    pub qualifier: McObjectQualifier,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub factor: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<f64>,

    #[serde(default)]
    #[serde(skip_serializing_if = "McText::is_empty")]
    pub unit: McText,

    #[serde(default)]
    #[serde(skip_serializing_if = "McText::is_empty")]
    pub comment: McText,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub x_axis_ref: Option<McIdentifier>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub y_axis_ref: Option<McIdentifier>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub x_axis_conv: Option<McIdentifier>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub y_axis_conv: Option<McIdentifier>,
}

impl Default for McSupportData {
    fn default() -> Self {
        McSupportData {
            object_type: McObjectType::Unspecified,
            qualifier: McObjectQualifier::Unspecified,
            factor: None,
            offset: None,
            unit: McText::default(),
            comment: McText::default(),
            min: None,
            max: None,
            step: None,
            x_axis_ref: None,
            y_axis_ref: None,
            x_axis_conv: None,
            y_axis_conv: None,
        }
    }
}

impl Clone for McSupportData {
    fn clone(&self) -> Self {
        log::debug!("Cloning McSupportData");
        McSupportData {
            object_type: self.object_type,
            qualifier: self.qualifier,
            factor: self.factor,
            offset: self.offset,
            unit: self.unit,
            comment: self.comment,
            min: self.min,
            max: self.max,
            step: self.step,
            x_axis_ref: self.x_axis_ref,
            y_axis_ref: self.y_axis_ref,
            x_axis_conv: self.x_axis_conv,
            y_axis_conv: self.y_axis_conv,
        }
    }
}

impl std::fmt::Display for McSupportData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "McSupportData:")?;
        write!(f, " {:?}", self.object_type)?;
        if self.qualifier != McObjectQualifier::Unspecified {
            write!(f, " {:?}", self.qualifier)?;
        }
        if self.factor.is_some() {
            write!(f, " factor={}", self.factor.unwrap())?;
        }
        if self.offset.is_some() {
            write!(f, " offset={}", self.offset.unwrap())?;
        }
        if !self.unit.is_empty() {
            write!(f, " unit={}", self.unit)?;
        }
        if self.min.is_some() {
            write!(f, " min={}", self.min.unwrap())?;
        }
        if self.max.is_some() {
            write!(f, " max={}", self.max.unwrap())?;
        }
        if self.step.is_some() {
            write!(f, " step={}", self.step.unwrap())?;
        }
        if self.x_axis_ref.is_some() {
            write!(f, " x_axis_ref={}", self.x_axis_ref.as_ref().unwrap())?;
        }
        if self.y_axis_ref.is_some() {
            write!(f, " y_axis_ref={}", self.y_axis_ref.as_ref().unwrap())?;
        }
        if self.x_axis_conv.is_some() {
            write!(f, " x_axis_conv={}", self.x_axis_conv.as_ref().unwrap())?;
        }
        if self.y_axis_conv.is_some() {
            write!(f, " y_axis_conv={}", self.y_axis_conv.as_ref().unwrap())?;
        }

        if !self.comment.is_empty() {
            write!(f, " {}", self.comment)?;
        }
        Ok(())
    }
}

impl McSupportData {
    pub fn new(object_type: McObjectType) -> Self {
        McSupportData {
            object_type,
            qualifier: McObjectQualifier::Unspecified,
            factor: None,
            offset: None,
            unit: McText::default(),
            comment: McText::default(),
            min: None,
            max: None,
            step: None,
            x_axis_ref: None,
            y_axis_ref: None,
            x_axis_conv: None,
            y_axis_conv: None,
        }
    }

    // Read and write json string

    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap()
    }

    pub fn from_json_string(s: &str) -> Option<Self> {
        match serde_json::from_str(s) {
            Ok(m) => Some(m),
            Err(e) => {
                log::error!("McSupportData from json failed: {}", e);
                None
            }
        }
    }

    // Conversion rule
    pub fn convert(&self, value: f64) -> f64 {
        let mut result = value;
        // physical_value = value * factor + offset !!
        if let Some(factor) = self.factor {
            result *= factor;
        }
        if let Some(offset) = self.offset {
            result += offset;
        }
        result
    }

    // Setters for builder syntax
    pub fn set_object_type(mut self, object_type: McObjectType) -> Self {
        assert!(object_type != McObjectType::Unspecified);
        self.object_type = object_type;
        self
    }
    pub fn set_qualifier(mut self, qualifier: McObjectQualifier) -> Self {
        self.qualifier = qualifier;
        self
    }

    pub fn set_linear<T: Into<McText>>(mut self, factor: f64, offset: f64, unit: T) -> Self {
        self.unit = unit.into();
        self.factor = if (factor - 1.0).abs() > f64::EPSILON { Some(factor) } else { None };
        self.offset = if offset.abs() > f64::EPSILON { Some(offset) } else { None };
        self
    }
    pub fn set_factor(mut self, factor: Option<f64>) -> Self {
        if let Some(factor) = factor {
            self.factor = if (factor - 1.0).abs() > f64::EPSILON { Some(factor) } else { None };
            return self;
        }
        self.factor = None;
        self
    }
    pub fn set_offset(mut self, offset: Option<f64>) -> Self {
        if let Some(offset) = offset {
            self.offset = if offset.abs() > f64::EPSILON { Some(offset) } else { None };
            return self;
        }
        self.offset = None;
        self
    }

    pub fn set_unit<T: Into<McText>>(mut self, unit: T) -> Self {
        self.unit = unit.into();
        self
    }
    pub fn set_comment<T: Into<McText>>(mut self, comment: T) -> Self {
        self.comment = comment.into();
        self
    }
    pub fn set_min(mut self, min: Option<f64>) -> Self {
        self.min = min;
        self
    }
    pub fn set_max(mut self, max: Option<f64>) -> Self {
        self.max = max;
        self
    }
    pub fn set_step(mut self, step: Option<f64>) -> Self {
        self.step = step;
        self
    }
    pub fn set_x_axis_ref<T: Into<McIdentifier>>(mut self, x_axis_ref: Option<T>) -> Self {
        self.x_axis_ref = x_axis_ref.map(|s| s.into());
        self
    }
    pub fn set_y_axis_ref<T: Into<McIdentifier>>(mut self, y_axis_ref: Option<T>) -> Self {
        self.y_axis_ref = y_axis_ref.map(|s| s.into());
        self
    }
    pub fn set_x_axis_conv<T: Into<McIdentifier>>(mut self, x_axis_conv: Option<T>) -> Self {
        self.x_axis_conv = x_axis_conv.map(|s| s.into());
        self
    }
    pub fn set_y_axis_conv<T: Into<McIdentifier>>(mut self, y_axis_conv: Option<T>) -> Self {
        self.y_axis_conv = y_axis_conv.map(|s| s.into());
        self
    }
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
// Test module

#[cfg(test)]
mod mc_support_data_tests {

    use super::*;

    #[test]
    fn test_mc_support_data() {
        let _ = crate::xcp::xcp_test::test_setup();

        let m1 = McSupportData::new(McObjectType::Characteristic)
            .set_min(Some(0.0))
            .set_max(Some(100.0))
            .set_step(Some(1.0))
            .set_unit("Volts")
            .set_comment("Voltage in Volts");

        let m2 = m1.clone();
        assert_eq!(m2.object_type, McObjectType::Characteristic);
        assert_eq!(m2.min, Some(0.0));
        assert_eq!(m2.max, Some(100.0));
        assert_eq!(m2.step, Some(1.0));
        assert_eq!(m2.unit.as_str(), "Volts");
        assert_eq!(m2.comment.as_str(), "Voltage in Volts");

        // Serialize, Deserialize
        let m1 = McSupportData::new(McObjectType::Characteristic)
            .set_min(Some(0.0))
            .set_max(Some(100.0))
            .set_step(Some(1.0))
            .set_unit("Json-Unit")
            .set_comment("Json-Comment");
        let s = m1.to_json_string();
        let m2 = McSupportData::from_json_string(&s).unwrap();
        assert_eq!(m2.object_type, McObjectType::Characteristic);
        assert_eq!(m2.min, Some(0.0));
        assert_eq!(m2.max, Some(100.0));
        assert_eq!(m2.unit.as_str(), "Json-Unit");
        assert_eq!(m2.comment.as_str(), "Json-Comment");
        let s1 = r#"{
                "object_type":"Characteristic",
                "unit":"Json-Unit",
                "comment":"Json-Comment",
                "min":0.0,
                "max":50.0,
                "step":1.0
        }"#;
        let m3 = McSupportData::from_json_string(s1).unwrap();
        log::debug!("m3: {m2}");
        assert_eq!(m3.object_type, McObjectType::Characteristic);
        assert_eq!(m3.min, Some(0.0));
        assert_eq!(m3.max, Some(50.0));
        assert_eq!(m3.step, Some(1.0));
        assert_eq!(m3.factor, None);
        assert_eq!(m3.unit.as_str(), "Json-Unit");
        assert_eq!(m3.comment.as_str(), "Json-Comment");
    }
}
