// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Canonical C Query parsing and resumable matching.
use crate::{Element, Error, Result, Tag, TreeReader, Visit};
use opentlv_sys as native;
use std::{mem::MaybeUninit, ptr};

/// A parsed path owning its tags, independent of the original query string.
#[derive(Clone, Debug)]
pub struct Query {
    pub(crate) raw: native::tlv_query_t,
}

/// Query parse failure with the original text offset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryError {
    /// Original C result code.
    pub error: Error,
    /// Byte offset in the query text.
    pub location: crate::Location,
}
impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at {:?}", self.error, self.location)
    }
}
impl std::error::Error for QueryError {}

impl Query {
    /// Parse using C's path grammar; embedded NUL is rejected, never truncated.
    pub fn parse(text: &str) -> std::result::Result<Self, QueryError> {
        let mut raw = MaybeUninit::uninit();
        // SAFETY: all-zero native diagnostics have no active borrowed fields.
        let mut diagnostic: native::tlv_diagnostic_t = unsafe { std::mem::zeroed() };
        // SAFETY: Readable bounded UTF-8 input and writable outputs.
        let code = unsafe {
            native::tlv_query_parse_n(
                text.as_ptr().cast(),
                text.len(),
                raw.as_mut_ptr(),
                &mut diagnostic,
            )
        };
        Error::check(code).map_err(|error| QueryError {
            error,
            location: crate::Location::from_raw(diagnostic.location),
        })?;
        // SAFETY: successful C parse initialized all storage.
        Ok(Self {
            raw: unsafe { raw.assume_init() },
        })
    }
    /// Number of path components.
    pub fn len(&self) -> usize {
        // SAFETY: initialized Query borrowed for the call.
        unsafe { native::tlv_query_count(&self.raw) }
    }
    /// A parsed query is nonempty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Return the canonical uppercase V1 path using the native formatter.
    pub fn format(&self) -> Result<String> {
        let mut required = 0;
        // SAFETY: initialized Query and writable discovery output.
        Error::check(unsafe {
            native::tlv_query_format(&self.raw, ptr::null_mut(), 0, &mut required)
        })?;
        let mut output = vec![0; required];
        // SAFETY: discovery supplied the bounded output size including NUL.
        Error::check(unsafe {
            native::tlv_query_format(
                &self.raw,
                output.as_mut_ptr().cast(),
                output.len(),
                &mut required,
            )
        })?;
        output.pop();
        String::from_utf8(output).map_err(|_| Error::InvalidValue)
    }
    /// Returns an owned tag at a path step, or None out of bounds.
    pub fn step(&self, index: usize) -> Option<Tag> {
        if index >= self.len() {
            return None;
        }
        // SAFETY: native step borrows self; Tag copies it before returning.
        unsafe { Tag::from_raw(&native::tlv_query_step(&self.raw, index)).ok() }
    }
    /// Start independent matching with an owned, stable copy of this query.
    pub fn matcher(&self) -> Result<QueryMatcher> {
        QueryMatcher::new(self)
    }
}

/// Resumable matcher; owns stable query storage and delegates all matching to C.
pub struct QueryMatcher {
    raw: native::tlv_query_matcher_t,
    _query: Box<native::tlv_query_t>,
}
impl QueryMatcher {
    /// Create a matcher for the start of one preorder traversal.
    pub fn new(query: &Query) -> Result<Self> {
        let owned = Box::new(query.raw);
        let mut raw = MaybeUninit::uninit();
        // SAFETY: stable owned heap query and writable output.
        Error::check(unsafe { native::tlv_query_matcher_init(raw.as_mut_ptr(), &*owned) })?;
        // SAFETY: successful initialization.
        Ok(Self {
            raw: unsafe { raw.assume_init() },
            _query: owned,
        })
    }
    /// Feed every preorder item to the C matcher, including nonmatching ancestors.
    pub fn matches(&mut self, tag: &Tag, depth: usize) -> bool {
        // SAFETY: initialized matcher and valid borrowed tag for the synchronous call.
        unsafe { native::tlv_query_matcher_visit(&mut self.raw, &tag.raw(), depth) != 0 }
    }
    /// Reset matching for another preorder traversal of the same path.
    pub fn reset(&mut self) -> Result<()> {
        // SAFETY: exclusive matcher and its stable, live owned Query.
        Error::check(unsafe { native::tlv_query_matcher_init(&mut self.raw, &*self._query) })
    }
    /// Replace the owned path with an equivalent copy, preserving continuation.
    /// A different path is rejected without changing the matcher.
    pub fn rebind(&mut self, query: &Query) -> Result<()> {
        let replacement = Box::new(query.raw);
        // SAFETY: both old and replacement Query storage remain live through this call.
        Error::check(unsafe { native::tlv_query_matcher_rebind(&mut self.raw, &*replacement) })?;
        self._query = replacement;
        Ok(())
    }
    /// Visit matches from a Tree Reader, retaining state across STOP and input replacement.
    /// Do not interleave unmatched pulls. Panics resume after C returns.
    pub fn visit<'a>(
        &mut self,
        reader: &mut TreeReader<'a>,
        callback: impl FnMut(Element<'a>, usize, usize) -> Visit,
    ) -> Result<()> {
        // SAFETY: exclusive reader/matcher, owned frames/query and live 'a input.
        reader.current = None;
        let (result, diagnostic) = unsafe {
            crate::visitor::run(ptr::null_mut(), &mut reader.raw, callback, &mut self.raw)
        };
        reader.diagnostic = diagnostic;
        result
    }
}
