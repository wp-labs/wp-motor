use once_cell::sync::OnceCell;
use smol_str::SmolStr;
use std::{fmt, net::IpAddr, sync::Arc};

use crate::traits::{FieldProcessor, FiledExtendType, first_field_processor};

#[derive(Clone, Debug, PartialEq)]
pub struct CharsValue(pub(crate) SmolStr);

// ============ Field Existence Check ============

/// Checks if active field exists
#[derive(Clone, Debug, PartialEq)]
pub struct Has;

/// Checks if a specified target field exists
#[derive(Clone, Debug, PartialEq)]
pub struct TargetHas {
    pub(crate) target: Option<SmolStr>,
}

impl TargetHas {
    pub fn new<S: Into<SmolStr>>(target: S) -> Self {
        Self {
            target: Some(target.into()),
        }
    }

    pub fn for_active_field() -> Self {
        Self { target: None }
    }
}

// ============ Character String Operations ============

/// Checks if active field's character value equals a specific string
#[derive(Clone, Debug, PartialEq)]
pub struct CharsHas {
    pub(crate) value: SmolStr,
}

/// Checks if specified field's character value equals a specific string
#[derive(Clone, Debug, PartialEq)]
pub struct TargetCharsHas {
    pub(crate) target: Option<SmolStr>,
    pub(crate) value: SmolStr,
}

/// Checks if active field's character value does NOT equal a specific string
#[derive(Clone, Debug, PartialEq)]
pub struct CharsNotHas {
    pub(crate) value: SmolStr,
}

/// Checks if specified field's character value does NOT equal a specific string
#[derive(Clone, Debug, PartialEq)]
pub struct TargetCharsNotHas {
    pub(crate) target: Option<SmolStr>,
    pub(crate) value: SmolStr,
}

/// Checks if active field's character value is in a list
#[derive(Clone, Debug, PartialEq)]
pub struct CharsIn {
    pub(crate) value: Vec<SmolStr>,
}

/// Checks if specified field's character value is in a list
#[derive(Clone, Debug, PartialEq)]
pub struct TargetCharsIn {
    pub(crate) target: Option<SmolStr>,
    pub(crate) value: Vec<SmolStr>,
}

// ============ Numeric Operations ============

/// Checks if active field's numeric value equals a specific number
#[derive(Clone, Debug, PartialEq)]
pub struct DigitHas {
    pub(crate) value: i64,
}

/// Checks if specified field's numeric value equals a specific number
#[derive(Clone, Debug, PartialEq)]
pub struct TargetDigitHas {
    pub(crate) target: Option<SmolStr>,
    pub(crate) value: i64,
}

/// Checks if active field's numeric value is in a list
#[derive(Clone, Debug, PartialEq)]
pub struct DigitIn {
    pub(crate) value: Vec<i64>,
}

/// Checks if specified field's numeric value is in a list
#[derive(Clone, Debug, PartialEq)]
pub struct TargetDigitIn {
    pub(crate) target: Option<SmolStr>,
    pub(crate) value: Vec<i64>,
}

// ============ IP Address Operations ============

/// Checks if active field's IP address is in a list
#[derive(Clone, Debug, PartialEq)]
pub struct IpIn {
    pub(crate) value: Vec<IpAddr>,
}

/// Checks if specified field's IP address is in a list
#[derive(Clone, Debug, PartialEq)]
pub struct TargetIpIn {
    pub(crate) target: Option<SmolStr>,
    pub(crate) value: Vec<IpAddr>,
}

// ============ Legacy/Compatibility ============

#[derive(Clone, Default)]
#[allow(dead_code)]
pub struct StubFun {}

// ============ Transformation Functions ============

#[derive(Clone, Debug, PartialEq)]
pub struct JsonUnescape {}

#[derive(Clone, Debug, PartialEq)]
pub struct Base64Decode {}

#[derive(Clone)]
pub struct ExtPassFunc {
    pub(crate) extend_type: FiledExtendType,
    pub(crate) processor: Arc<dyn FieldProcessor>,
}

#[derive(Clone)]
pub struct VecToSrcFunc {
    pub(crate) extend_type: FiledExtendType,
    pub(crate) processor: Arc<dyn FieldProcessor>,
}

#[derive(Clone)]
pub struct SplitInnerSrcFunc {
    sep_op: SmolStr,
    processor: Arc<OnceCell<Arc<dyn FieldProcessor>>>,
}

impl ExtPassFunc {
    pub fn new(extend_type: FiledExtendType, processor: Arc<dyn FieldProcessor>) -> Self {
        Self {
            extend_type,
            processor,
        }
    }

    pub fn from_registry(extend_type: FiledExtendType) -> Option<Self> {
        first_field_processor(extend_type).map(|proc| Self::new(extend_type, proc))
    }
}

impl VecToSrcFunc {
    pub fn new(extend_type: FiledExtendType, processor: Arc<dyn FieldProcessor>) -> Self {
        Self {
            extend_type,
            processor,
        }
    }

    pub fn from_registry(extend_type: FiledExtendType) -> Option<Self> {
        first_field_processor(extend_type).map(|proc| Self::new(extend_type, proc))
    }
}

impl SplitInnerSrcFunc {
    pub fn new(separator: SmolStr) -> Self {
        Self {
            sep_op: separator,
            processor: Arc::new(OnceCell::new()),
        }
    }

    pub fn separator(&self) -> &SmolStr {
        &self.sep_op
    }

    pub fn processor(&self) -> Option<Arc<dyn FieldProcessor>> {
        if let Some(proc) = self.processor.get() {
            return Some(proc.clone());
        }
        first_field_processor(FiledExtendType::InnerSource).map(|proc| {
            let _ = self.processor.set(proc.clone());
            proc
        })
    }
}

impl fmt::Debug for ExtPassFunc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExtPassFunc")
            .field("extend_type", &self.extend_type)
            .finish()
    }
}

impl fmt::Debug for SplitInnerSrcFunc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SplitInnerSrcFunc")
            .field("separator", &self.sep_op)
            .finish()
    }
}

impl fmt::Debug for VecToSrcFunc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VecToSrcFunc")
            .field("extend_type", &self.extend_type)
            .finish()
    }
}

impl PartialEq for ExtPassFunc {
    fn eq(&self, other: &Self) -> bool {
        self.extend_type == other.extend_type
    }
}

impl PartialEq for SplitInnerSrcFunc {
    fn eq(&self, other: &Self) -> bool {
        self.sep_op == other.sep_op
    }
}

impl PartialEq for VecToSrcFunc {
    fn eq(&self, other: &Self) -> bool {
        self.extend_type == other.extend_type
    }
}
// ============ Field Selector Functions ============

#[derive(Clone, Debug, PartialEq)]
pub struct TakeField {
    pub(crate) target: SmolStr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SelectLast {}

// ============ Parser Helper - Arguments Only ============
// These are only used when we need temporary types during parsing that differ from the final struct

/// Parser argument for `chars_not_has(value)` - converted to CharsNotHas
#[derive(Clone, Debug, PartialEq)]
pub struct CharsNotHasArg {
    pub(crate) value: SmolStr,
}

/// Parser argument for `chars_in([...])` - converted to CharsIn
#[derive(Clone, Debug, PartialEq)]
pub struct CharsInArg {
    pub(crate) value: Vec<SmolStr>,
}

/// Parser argument for `digit_has(num)` - converted to DigitHas
#[derive(Clone, Debug, PartialEq)]
pub struct DigitHasArg {
    pub(crate) value: i64,
}

/// Parser argument for `digit_in([...])` - converted to DigitIn
#[derive(Clone, Debug, PartialEq)]
pub struct DigitInArg {
    pub(crate) value: Vec<i64>,
}

/// Parser argument for `ip_in([...])` - converted to IpIn
#[derive(Clone, Debug, PartialEq)]
pub struct IpInArg {
    pub(crate) value: Vec<IpAddr>,
}

/// Parser argument for `has()` - converted to Has
#[derive(Clone, Debug, PartialEq)]
pub struct HasArg;

/// Normalizes the target field name: converts "_" to None
pub(crate) fn normalize_target(target: SmolStr) -> Option<SmolStr> {
    if target == "_" { None } else { Some(target) }
}
