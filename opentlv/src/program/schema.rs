// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel
//! Owning storage adaptation for canonical contextual Query Schema validation.
use super::*;

/// One contextual assertion, owning shared immutable selector/assertion programs.
/// The assertion's environment supplies compatible providers to both programs.
#[derive(Clone)]
pub struct QueryRule {
    /// Node selector evaluated against the original input root.
    pub context: QueryProgram,
    /// Boolean assertion evaluated relative to each selected context.
    pub assertion: QueryProgram,
    /// Optional owned diagnostic field name.
    pub name: Option<CString>,
}
impl QueryRule {
    /// Retain two programs. Native validation checks their result types and providers.
    pub fn new(context: QueryProgram, assertion: QueryProgram) -> Self {
        Self {
            context,
            assertion,
            name: None,
        }
    }
    /// Set the diagnostic field name, rejecting embedded NUL bytes.
    pub fn named(mut self, name: &str) -> Result<Self, std::ffi::NulError> {
        self.name = Some(CString::new(name)?);
        Ok(self)
    }
}

/// Explicit bounds for each selector/assertion execution and owning workspace.
#[derive(Clone, Copy, Debug)]
pub struct QuerySchemaLimits {
    /// Maximum traversal depth.
    pub depth: usize,
    /// Retained node capacity, including nonmatches.
    pub nodes: usize,
    /// Native operation budget per selector and per assertion context.
    pub work: usize,
    /// Selected contexts retained per rule, including overlaps.
    pub contexts: usize,
    /// Document Value snapshot bytes; None measures the immutable Document.
    pub value_capacity: Option<usize>,
}
impl Default for QuerySchemaLimits {
    fn default() -> Self {
        Self {
            depth: 64,
            nodes: 1024,
            work: 100_000_000,
            contexts: 1024,
            value_capacity: None,
        }
    }
}

/// Owned Schema detail of a false contextual assertion.
///
/// The common evidence of the failure, such as its location, path and expected
/// text, is the Query failure's own and is read from [`QuerySchemaError::failure`].
#[derive(Clone, Debug)]
pub struct QuerySchemaDetail {
    /// Canonical violation kind, normally [`crate::SchemaIssue::Assertion`].
    pub kind: crate::SchemaIssue,
    /// Tag of the selected context.
    pub tag: crate::Tag,
    /// Owned rule name, when the rule has one.
    pub field: Option<String>,
}

/// Owned rule-aware failure, independent of input, Document and program lifetimes.
#[derive(Clone, Debug)]
pub struct QuerySchemaError {
    /// Zero-based failing rule; zero also identifies failures before rule execution.
    pub rule: usize,
    /// Original Query, Reader, codec or storage error, including the common evidence.
    pub failure: Box<ProgramError>,
    /// Schema detail when a boolean assertion was false.
    pub schema: Option<Box<QuerySchemaDetail>>,
}
impl From<ProgramError> for QuerySchemaError {
    fn from(failure: ProgramError) -> Self {
        Self {
            rule: 0,
            failure: Box::new(failure),
            schema: None,
        }
    }
}
impl std::fmt::Display for QuerySchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Query Schema rule {}: {}", self.rule, self.failure)
    }
}
impl std::error::Error for QuerySchemaError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.failure.as_ref())
    }
}

/// Immutable owning contextual rules. Each validation owns independent bounded
/// storage and delegates all selection, context and assertion semantics to C.
#[derive(Clone)]
pub struct QuerySchema {
    rules: Vec<QueryRule>,
    format: FormatOwner,
}
impl QuerySchema {
    /// Own rules and a builtin wire Format. Empty rules explicitly skip validation.
    pub fn new(rules: Vec<QueryRule>, format: Format) -> Self {
        Self {
            rules,
            format: FormatOwner::Builtin(format),
        }
    }
    /// Own contextual rules and a shared configurable Fixed descriptor.
    pub fn with_fixed_format(rules: Vec<QueryRule>, format: crate::OwnedFixedFormat) -> Self {
        Self {
            rules,
            format: FormatOwner::Fixed(format),
        }
    }
    /// Own contextual rules and a shared composed descriptor. Compile the rules
    /// with a clone of the same owner, as with a Fixed descriptor.
    pub fn with_composed_format(rules: Vec<QueryRule>, format: crate::ComposedFormat) -> Self {
        Self {
            rules,
            format: FormatOwner::Composed(format),
        }
    }
    /// Borrow the immutable retained rule table.
    pub fn rules(&self) -> &[QueryRule] {
        &self.rules
    }
    /// Validate complete input, preserving structural Reader failures. Document-only
    /// programs are rejected by C before traversal. Input stays borrowed throughout.
    pub fn validate_buffer(
        &self,
        input: &[u8],
        limits: QuerySchemaLimits,
    ) -> Result<(), QuerySchemaError> {
        if self.rules.is_empty() {
            return Ok(());
        }
        let mut storage = Workspace::new(&self.rules, limits)?;
        let mut workspace = storage.raw();
        let mut diagnostic = unsafe { zeroed() };
        // SAFETY: all programs, input, callbacks and disjoint workspace stay alive
        // until the synchronous C call and diagnostic projection have completed.
        let rc = unsafe {
            native::tlv_schema_query_validate_buffer(
                input.as_ptr(),
                input.len(),
                self.format.raw(),
                storage.rules.as_ptr(),
                storage.rules.len(),
                limits.depth,
                limits.nodes,
                limits.work,
                &mut workspace,
                &mut diagnostic,
            )
        };
        schema_result(rc, &diagnostic)
    }
    /// Validate an immutable Document revision, including reverse axes. The shared
    /// borrow prevents safe mutation or destruction while provider callbacks run.
    /// Document diagnostics deliberately have no historical Source offset.
    #[cfg(feature = "document")]
    pub fn validate_document(
        &self,
        document: &crate::Document<'_>,
        limits: QuerySchemaLimits,
    ) -> Result<(), QuerySchemaError> {
        if self.rules.is_empty() {
            return Ok(());
        }
        let mut storage = Workspace::new(&self.rules, limits)?;
        let mut workspace = storage.raw();
        let need_values = self.rules.iter().any(|rule| {
            rule.context.info().constructed_values_required != 0
                || rule.assertion.info().constructed_values_required != 0
        });
        let capacity = if need_values {
            match limits.value_capacity {
                Some(value) => value,
                None => document
                    .encoded_size()
                    .map_err(|error| plain(error.code()).unwrap_err())?,
            }
        } else {
            0
        };
        let mut values = Memory::new(capacity)?;
        let mut staged = Memory::new(capacity)?;
        let mut scratch = Memory::new(capacity)?;
        let mut frames = zeroed_vec::<native::tlv_tree_writer_frame_t>(if need_values {
            storage.frames.len()
        } else {
            0
        })?;
        let mut staging = native::tlv_tree_writer_workspace_t {
            frames: frames.as_mut_ptr(),
            frame_capacity: frames.len(),
            data: staged.data_mut().cast(),
            data_capacity: capacity,
            scratch: scratch.data_mut().cast(),
            scratch_capacity: capacity,
            required_data: 0,
            required_scratch: 0,
        };
        let mut diagnostic = unsafe { zeroed() };
        // SAFETY: &Document excludes writes, all native spans are retained through
        // the call, and every mutable workspace belongs exclusively to this call.
        let rc = unsafe {
            native::tlv_schema_query_validate_document(
                document.raw,
                storage.rules.as_ptr(),
                storage.rules.len(),
                limits.depth,
                limits.nodes,
                limits.work,
                &mut workspace,
                values.data_mut().cast(),
                capacity,
                if need_values {
                    &mut staging
                } else {
                    ptr::null_mut()
                },
                &mut diagnostic,
            )
        };
        schema_result(rc, &diagnostic)
    }
}

fn schema_result(
    code: i32,
    diagnostic: &native::tlv_schema_query_diagnostic_t,
) -> Result<(), QuerySchemaError> {
    check(code, &diagnostic.query).map_err(|failure| {
        // SAFETY: input, Document and rule names are still alive. Only a false
        // assertion selects the Schema cause; its borrowed fields are copied.
        let schema = if diagnostic.query.cause == native::TLV_QUERY_CAUSE_SCHEMA {
            let detail = unsafe { diagnostic.query.detail.schema };
            unsafe { crate::Tag::from_raw(&detail.tag) }
                .ok()
                .map(|tag| {
                    Box::new(QuerySchemaDetail {
                        kind: crate::SchemaIssue::from_raw(detail.kind),
                        tag,
                        field: (!detail.field.is_null()).then(|| {
                            unsafe { CStr::from_ptr(detail.field) }
                                .to_string_lossy()
                                .into_owned()
                        }),
                    })
                })
        } else {
            None
        };
        QuerySchemaError {
            rule: diagnostic.rule,
            failure: Box::new(failure),
            schema,
        }
    })
}

// Only invoked for native plain-data records whose fields admit all-zero values.
fn zeroed_vec<T>(count: usize) -> ProgramResult<Vec<T>> {
    count
        .checked_mul(size_of::<T>())
        .filter(|size| *size <= isize::MAX as usize)
        .ok_or_else(|| plain(native::TLV_ERR_OVERFLOW).unwrap_err())?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| plain(native::TLV_ERR_OUT_OF_MEMORY).unwrap_err())?;
    result.resize_with(count, || unsafe { zeroed() });
    Ok(result)
}
struct Workspace {
    rules: Vec<native::tlv_schema_query_rule_t>,
    selector: Memory,
    assertion: Memory,
    contexts: Vec<native::tlv_schema_query_context_t>,
    frames: Vec<native::tlv_tree_frame_t>,
}
impl Workspace {
    fn new(rules: &[QueryRule], limits: QuerySchemaLimits) -> ProgramResult<Self> {
        let frame_count = limits
            .depth
            .checked_add(1)
            .ok_or_else(|| plain(native::TLV_ERR_OVERFLOW).unwrap_err())?;
        let mut raw_rules = zeroed_vec::<native::tlv_schema_query_rule_t>(rules.len())?;
        for (raw, rule) in raw_rules.iter_mut().zip(rules) {
            *raw = native::tlv_schema_query_rule_t {
                context: rule.context.storage.memory.data().cast(),
                assertion: rule.assertion.storage.memory.data().cast(),
                environment: &rule.assertion.storage.environment,
                name: rule.name.as_ref().map_or(ptr::null(), |name| name.as_ptr()),
            };
        }
        let (mut selector, mut assertion, mut alignment) = (0, 0, 0);
        // SAFETY: borrowed descriptors reference programs retained by the Schema.
        plain(unsafe {
            native::tlv_schema_query_size(
                raw_rules.as_ptr(),
                raw_rules.len(),
                limits.depth,
                limits.nodes,
                &mut selector,
                &mut assertion,
                &mut alignment,
            )
        })?;
        if alignment > std::mem::align_of::<Block>() {
            return Err(plain(native::TLV_ERR_INVALID_ARG).unwrap_err());
        }
        Ok(Self {
            rules: raw_rules,
            selector: Memory::new(selector)?,
            assertion: Memory::new(assertion)?,
            contexts: zeroed_vec(limits.contexts)?,
            frames: zeroed_vec(frame_count)?,
        })
    }
    fn raw(&mut self) -> native::tlv_schema_query_workspace_t {
        native::tlv_schema_query_workspace_t {
            selector: self.selector.data_mut(),
            selector_size: self.selector.bytes,
            assertion: self.assertion.data_mut(),
            assertion_size: self.assertion.bytes,
            contexts: self.contexts.as_mut_ptr(),
            context_capacity: self.contexts.len(),
            frames: self.frames.as_mut_ptr(),
            frame_capacity: self.frames.len(),
        }
    }
}
