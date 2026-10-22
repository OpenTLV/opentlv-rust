// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Generic identifier metadata; lookup semantics belong to the C registry.
use crate::Tag;
use opentlv_sys as native;
use std::ffi::CString;

/// Identifier and optional descriptive name; no Schema or Codec is implied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Definition {
    /// Canonical identifier bytes, independent of raw wire headers.
    pub tag: Tag,
    /// Owned NUL-terminated descriptive name, if any.
    pub name: Option<CString>,
}
impl Definition {
    /// Create an unnamed definition.
    pub fn new(tag: Tag) -> Self {
        Self { tag, name: None }
    }
    /// Set a descriptive name, rejecting embedded NUL bytes.
    pub fn named(mut self, name: &str) -> Result<Self, std::ffi::NulError> {
        self.name = Some(CString::new(name)?);
        Ok(self)
    }
}

/// Owned immutable registry, retaining identifier and name storage across moves.
/// Repeated identifiers use the first entry, as defined by C.
pub struct DefinitionRegistry {
    definitions: Vec<Definition>,
    entries: Vec<native::tlv_definition_t>,
}
impl DefinitionRegistry {
    pub(crate) fn raw(&self) -> native::tlv_definition_registry_t {
        native::tlv_definition_registry_t {
            entries: self.entries.as_ptr(),
            count: self.entries.len(),
        }
    }
    /// Build a registry without parsing, encoding or resolving protocol context.
    pub fn new(definitions: impl IntoIterator<Item = Definition>) -> Self {
        let definitions: Vec<_> = definitions.into_iter().collect();
        let entries = definitions
            .iter()
            .map(|definition| native::tlv_definition_t {
                tag: definition.tag.raw(),
                name: definition
                    .name
                    .as_ref()
                    .map_or(std::ptr::null(), |name| name.as_ptr()),
            })
            .collect();
        Self {
            definitions,
            entries,
        }
    }
    /// Resolve the first matching identifier using the canonical C lookup.
    pub fn find(&self, tag: &Tag) -> Option<&Definition> {
        let registry = native::tlv_definition_registry_t {
            entries: self.entries.as_ptr(),
            count: self.entries.len(),
        };
        // SAFETY: all immutable tables and identifier/name bytes remain live for this call.
        let found = unsafe { native::tlv_definition_find(&registry, &tag.raw()) };
        if found.is_null() {
            return None;
        }
        // SAFETY: C returns an entry inside this exact live table.
        let index = unsafe { found.offset_from(self.entries.as_ptr()) } as usize;
        self.definitions.get(index)
    }
    /// Borrow the registry's immutable definitions in input order.
    pub fn definitions(&self) -> &[Definition] {
        &self.definitions
    }
}
// SAFETY: owned immutable storage; the C pointers only reference retained heap allocations.
unsafe impl Send for DefinitionRegistry {}
unsafe impl Sync for DefinitionRegistry {}
