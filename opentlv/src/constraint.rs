// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Constraints on decoded scalar values, checked and applied by C.
use crate::{Error, Result};
use opentlv_sys as native;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

/// Constraint on one decoded scalar value, such as an ASN.1 INTEGER range or a
/// set of named numbers.
///
/// It applies to the value a codec decodes, not to raw Value bytes; lengths
/// and occurrences belong to [`StructureRule`](crate::StructureRule). C checks
/// the definition when it is built and validates each value.
///
/// ```
/// use opentlv::{Error, ValueConstraint};
///
/// let colour = ValueConstraint::named([(0, "red"), (1, "green")])?;
/// assert_eq!(colour.name(1), Some("green"));
/// assert_eq!(colour.validate(2), Err(Error::Schema));
/// assert_eq!(ValueConstraint::range(5, 1).unwrap_err(), Error::InvalidSchema);
/// # Ok::<(), Error>(())
/// ```
#[derive(Debug)]
pub struct ValueConstraint {
    raw: native::tlv_value_constraint_t,
    // The native definition borrows these heap buffers, which never move.
    values: Vec<i64>,
    names: Vec<Option<CString>>,
    _name_pointers: Vec<*const c_char>,
}

// SAFETY: immutable after construction; the native definition points only into
// buffers this value owns.
unsafe impl Send for ValueConstraint {}
unsafe impl Sync for ValueConstraint {}

impl ValueConstraint {
    /// Permits every value.
    pub fn any() -> Self {
        Self::build(
            native::TLV_VALUE_CONSTRAINT_NONE,
            0,
            0,
            Vec::new(),
            Vec::new(),
        )
        .expect("an unconstrained definition is valid")
    }

    /// Permits `min..=max`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidSchema`] if `min > max`.
    pub fn range(min: i64, max: i64) -> Result<Self> {
        Self::build(
            native::TLV_VALUE_CONSTRAINT_RANGE,
            min,
            max,
            Vec::new(),
            Vec::new(),
        )
    }

    /// Permits exactly the listed values.
    pub fn allowed(values: impl IntoIterator<Item = i64>) -> Result<Self> {
        Self::build(
            native::TLV_VALUE_CONSTRAINT_ALLOWED_VALUES,
            0,
            0,
            values.into_iter().collect(),
            Vec::new(),
        )
    }

    /// Permits the listed values, each with a display name such as an ASN.1
    /// named number.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidArg`] if a name contains a NUL byte.
    pub fn named<'n>(values: impl IntoIterator<Item = (i64, &'n str)>) -> Result<Self> {
        let (values, names): (Vec<_>, Vec<_>) = values
            .into_iter()
            .map(|(value, name)| CString::new(name).map(|name| (value, Some(name))))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| Error::InvalidArg)?
            .into_iter()
            .unzip();
        Self::build(
            native::TLV_VALUE_CONSTRAINT_ALLOWED_VALUES,
            0,
            0,
            values,
            names,
        )
    }

    fn build(
        kind: native::tlv_value_constraint_kind_t,
        min_value: i64,
        max_value: i64,
        values: Vec<i64>,
        names: Vec<Option<CString>>,
    ) -> Result<Self> {
        let name_pointers: Vec<_> = names
            .iter()
            .map(|name| name.as_ref().map_or(ptr::null(), |name| name.as_ptr()))
            .collect();
        let raw = native::tlv_value_constraint_t {
            kind,
            min_value,
            max_value,
            allowed_values: values.as_ptr(),
            allowed_values_count: values.len(),
            allowed_value_names: if names.is_empty() {
                ptr::null()
            } else {
                name_pointers.as_ptr()
            },
        };
        // SAFETY: the definition borrows the owned buffers for the call.
        Error::check(unsafe { native::tlv_value_constraint_check(&raw) })?;
        Ok(Self {
            raw,
            values,
            names,
            _name_pointers: name_pointers,
        })
    }

    /// Checks a decoded value.
    ///
    /// # Errors
    ///
    /// [`Error::Schema`] if the value violates the constraint.
    pub fn validate(&self, value: i64) -> Result<()> {
        // SAFETY: checked definition borrowing buffers owned by self.
        Error::check(unsafe { native::tlv_value_constraint_validate(&self.raw, value) })
    }

    /// Display name of an allowed value, or `None` when it is not listed or
    /// has no name.
    pub fn name(&self, value: i64) -> Option<&str> {
        // SAFETY: C returns NULL or one of the NUL-terminated names owned by self.
        let name = unsafe { native::tlv_value_constraint_name(&self.raw, value) };
        if name.is_null() {
            return None;
        }
        // SAFETY: non-null names point into self's owned UTF-8 CStrings.
        unsafe { CStr::from_ptr(name) }.to_str().ok()
    }
}

impl Clone for ValueConstraint {
    fn clone(&self) -> Self {
        Self::build(
            self.raw.kind,
            self.raw.min_value,
            self.raw.max_value,
            self.values.clone(),
            self.names.clone(),
        )
        .expect("a checked definition stays valid")
    }
}
