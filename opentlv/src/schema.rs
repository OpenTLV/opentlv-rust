// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Length schemas and structural schemas for validating TLV data.
//!
//! Both schema types wrap the C library's tables. Validation runs entirely in
//! C; this module only builds the borrowed C tables from owned Rust values and
//! maps the outcome to Rust errors.

use std::error;
use std::fmt;
use std::ptr;

use opentlv_sys as native;

use crate::emv::Context;
use crate::error::{Error, Result};
use crate::tag::Tag;

/// Length rule for one tag of a [`LengthSchema`].
///
/// Bounds are inclusive; equal bounds specify an exact length.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LengthRule {
    /// Tag the rule describes.
    pub tag: Tag,
    /// Minimum permitted value length in bytes.
    pub min_length: usize,
    /// Maximum permitted value length in bytes; `usize::MAX` is unrestricted.
    pub max_length: usize,
    /// C length policy bits; mask `1` permits only the two bounds.
    pub flags: u32,
    /// Required length multiple; zero disables the constraint.
    pub length_multiple: usize,
}

impl LengthRule {
    /// Creates a rule permitting lengths from `min_length` to `max_length`.
    pub fn new(tag: Tag, min_length: usize, max_length: usize) -> LengthRule {
        LengthRule {
            tag,
            min_length,
            max_length,
            flags: 0,
            length_multiple: 0,
        }
    }

    /// Creates a rule permitting exactly `length` bytes.
    pub fn exact(tag: Tag, length: usize) -> LengthRule {
        LengthRule::new(tag, length, length)
    }

    fn raw(&self) -> native::tlv_schema_entry_t {
        native::tlv_schema_entry_t {
            tag: self.tag.raw(),
            min_length: self.min_length,
            max_length: self.max_length,
            flags: self.flags,
            name: ptr::null(),
            length_multiple: self.length_multiple,
        }
    }

    /// # Safety
    ///
    /// `raw.tag` must reference readable bytes, as the entries of a live
    /// schema table do.
    unsafe fn from_raw(raw: &native::tlv_schema_entry_t) -> Result<LengthRule> {
        Ok(LengthRule {
            // SAFETY: the caller guarantees the tag bytes are readable.
            tag: unsafe { Tag::from_raw(&raw.tag) }?,
            min_length: raw.min_length,
            max_length: raw.max_length,
            flags: raw.flags,
            length_multiple: raw.length_multiple,
        })
    }
}

#[derive(Debug)]
enum Table {
    Owned {
        entries: Vec<native::tlv_schema_entry_t>,
        // Kept alive because `entries` borrows their bytes.
        _tags: Vec<Tag>,
    },
    Static(&'static native::tlv_schema_t),
}

/// A table of per-tag value-length rules.
///
/// Lookup and length checks are done by the C library. Use [`LengthSchema::new`]
/// for your own rules or [`LengthSchema::emv`] for the built-in EMV dictionary.
///
/// ```
/// use opentlv::{LengthRule, LengthSchema, Tag};
///
/// let tag = Tag::from_bytes(&[0x01]);
/// let schema = LengthSchema::new([LengthRule::new(tag.clone(), 2, 4)]);
/// assert!(schema.validate_length(&tag, 3).is_ok());
/// assert!(schema.validate_length(&tag, 5).is_err());
/// ```
#[derive(Debug)]
pub struct LengthSchema {
    table: Table,
}

// SAFETY: an owned table is plain data; a static table points to an immutable
// static that the C library never writes.
unsafe impl Send for LengthSchema {}
unsafe impl Sync for LengthSchema {}

impl LengthSchema {
    /// Creates a schema from `rules`. Earlier rules win for a repeated tag.
    pub fn new(rules: impl IntoIterator<Item = LengthRule>) -> LengthSchema {
        let mut entries = Vec::new();
        let mut tags = Vec::new();
        for rule in rules {
            entries.push(rule.raw());
            // Moving a tag moves its handle, not the heap bytes the entry borrows.
            tags.push(rule.tag);
        }
        LengthSchema {
            table: Table::Owned {
                entries,
                _tags: tags,
            },
        }
    }

    /// Returns the EMV dictionary length schema of the base context.
    pub fn emv() -> LengthSchema {
        LengthSchema::emv_for(Context::Base)
    }

    /// Returns the EMV dictionary length schema of `context`.
    pub fn emv_for(context: Context) -> LengthSchema {
        // SAFETY: every `Context` is a valid C context, so the call returns a
        // pointer to an immutable static table.
        let table = unsafe { native::tlv_emv_schema_for(context.raw()).as_ref() }
            .expect("every EMV context has a schema");
        LengthSchema {
            table: Table::Static(table),
        }
    }

    fn with_raw<R>(&self, f: impl FnOnce(&native::tlv_schema_t) -> R) -> R {
        match &self.table {
            Table::Owned { entries, .. } => f(&native::tlv_schema_t {
                entries: entries.as_ptr(),
                count: entries.len(),
            }),
            Table::Static(table) => f(table),
        }
    }

    /// Returns the number of rules.
    pub fn len(&self) -> usize {
        self.with_raw(|raw| raw.count)
    }

    /// Returns `true` if the schema has no rules.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn find_raw(&self, tag: &Tag) -> Option<native::tlv_schema_entry_t> {
        let tag = tag.raw();
        self.with_raw(|schema| {
            // SAFETY: `schema` and `tag` are valid for the call; the returned
            // pointer, if any, points into the table and is copied out
            // before the table can change.
            unsafe { native::tlv_schema_find(schema, &tag).as_ref().copied() }
        })
    }

    /// Returns the rule for `tag`, or `None` if the schema does not know it.
    pub fn find(&self, tag: &Tag) -> Option<LengthRule> {
        self.find_raw(tag)
            // SAFETY: the entry comes from this schema's table, whose tag bytes
            // are alive for as long as `self`.
            .and_then(|raw| unsafe { LengthRule::from_raw(&raw) }.ok())
    }

    /// Checks that a value of `length` bytes is permitted for `tag`.
    ///
    /// # Errors
    ///
    /// [`Error::Schema`] if the schema has no rule for `tag`,
    /// [`Error::Schema`] if the length is out of the rule's bounds,
    /// and [`Error::InvalidSchema`] for reversed bounds.
    pub fn validate_length(&self, tag: &Tag, length: usize) -> Result<()> {
        let entry = self.find_raw(tag).ok_or(Error::Schema)?;
        // SAFETY: `entry` is a valid, initialized entry.
        Error::check(unsafe { native::tlv_schema_validate_length(&entry, length) })
    }
}

/// Expected form of a value described by a [`StructureRule`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Kind {
    /// The element may be primitive or constructed.
    #[default]
    Any,
    /// The element must be primitive.
    Primitive,
    /// The element must be constructed.
    Constructed,
}

impl Kind {
    fn raw(self) -> native::tlv_schema_kind_t {
        match self {
            Kind::Any => native::TLV_SCHEMA_ANY,
            Kind::Primitive => native::TLV_SCHEMA_PRIMITIVE,
            Kind::Constructed => native::TLV_SCHEMA_CONSTRUCTED,
        }
    }
}

/// Structural rule for one tag within a single parent.
///
/// A new rule is optional (`0..=usize::MAX` occurrences), accepts any length
/// and any [`Kind`], and leaves the children unrestricted. Refine it with the
/// builder methods.
///
/// ```
/// use opentlv::{Kind, StructureRule, Tag};
///
/// let rule = StructureRule::new(Tag::from_bytes(&[0x84]))
///     .length(5, 16)
///     .required_once()
///     .kind(Kind::Primitive);
/// ```
#[derive(Debug)]
pub struct StructureRule {
    tag: Tag,
    min_length: usize,
    max_length: usize,
    min_occurs: usize,
    max_occurs: usize,
    kind: Kind,
    children: Option<StructureSchema>,
    flags: u32,
    length_multiple: usize,
    group: u32,
    name: Option<std::ffi::CString>,
}

impl StructureRule {
    /// Creates an optional rule for `tag` with no other restriction.
    pub fn new(tag: Tag) -> StructureRule {
        StructureRule {
            tag,
            min_length: 0,
            max_length: usize::MAX,
            min_occurs: 0,
            max_occurs: usize::MAX,
            kind: Kind::Any,
            children: None,
            flags: 0,
            length_multiple: 0,
            group: 0,
            name: None,
        }
    }

    /// Set the diagnostic field name, rejecting embedded NUL bytes.
    pub fn named(mut self, name: &str) -> std::result::Result<Self, std::ffi::NulError> {
        self.name = Some(std::ffi::CString::new(name)?);
        Ok(self)
    }

    /// Restricts the value length to `min..=max` bytes.
    pub fn length(mut self, min: usize, max: usize) -> StructureRule {
        self.min_length = min;
        self.max_length = max;
        self
    }

    /// Set native length policy flags (mask 1 permits only the two bounds).
    pub fn length_flags(mut self, flags: u32) -> Self {
        self.flags = flags;
        self
    }

    /// Require a Value length divisible by this width; zero disables it.
    pub fn length_multiple(mut self, width: usize) -> Self {
        self.length_multiple = width;
        self
    }

    /// Assign this rule to a native alternative group; zero means independent.
    /// Grouped rules must have minimum occurrences zero, as checked by C.
    pub fn group(mut self, id: u32) -> Self {
        self.group = id;
        self
    }

    /// Restricts the number of occurrences within the parent to `min..=max`.
    pub fn occurs(mut self, min: usize, max: usize) -> StructureRule {
        self.min_occurs = min;
        self.max_occurs = max;
        self
    }

    /// Requires exactly one occurrence within the parent.
    pub fn required_once(self) -> StructureRule {
        self.occurs(1, 1)
    }

    /// Requires the value to be primitive or constructed.
    pub fn kind(mut self, kind: Kind) -> StructureRule {
        self.kind = kind;
        self
    }

    /// Validates the value's children against `schema`.
    ///
    /// The C library requires a constructed value for this, so the kind is set
    /// to [`Kind::Constructed`].
    pub fn children(mut self, schema: StructureSchema) -> StructureRule {
        self.kind = Kind::Constructed;
        self.children = Some(schema);
        self
    }
}

/// The C tables behind an owned [`StructureSchema`]. Boxed so that the
/// addresses stored in parent tables stay valid when the schema is moved.
struct Compiled {
    rules: Vec<native::tlv_structure_rule_t>,
    // Finalized before rule pointers are formed; never resized afterwards.
    _fields: Vec<native::tlv_schema_entry_t>,
    // Kept alive because `rules` borrows their bytes.
    _tags: Vec<Tag>,
    // Kept alive because `rules` points to their C tables.
    _children: Vec<StructureSchema>,
    _groups: Vec<native::tlv_structure_group_t>,
    _names: Vec<std::ffi::CString>,
    raw: native::tlv_structure_schema_t,
}

enum Backing {
    Owned(Box<Compiled>),
    Static(&'static native::tlv_structure_schema_t),
}

/// A structural schema: per-tag length, kind, occurrence and membership rules
/// for one parent scope, with optional nested schemas for children.
///
/// [`StructureSchema::validate`] runs the C library's validator. Use
/// [`StructureSchema::new`] for your own rules or [`StructureSchema::emv`] for
/// the built-in EMV schema.
///
/// ```
/// use opentlv::{Format, StructureRule, StructureSchema, Tag, ValidationLimits};
///
/// let tag = Tag::from_bytes(&[0x01]);
/// let schema = StructureSchema::new([StructureRule::new(tag).required_once()], false);
/// let limits = ValidationLimits::default();
/// assert!(schema.validate(&[0x01, 0x00], Format::Ber, &limits).is_ok());
/// assert!(schema.validate(&[], Format::Ber, &limits).is_err());
/// ```
pub struct StructureSchema {
    backing: Backing,
}

/// Relative ordering of matched elements, enforced by the C Schema engine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SchemaOrder {
    /// No relative ordering constraint.
    #[default]
    Any,
    /// Matched elements follow their rule-table order.
    Sequence,
}

/// Unknown-tag policy for report-based validation.
#[derive(Clone, Copy, Debug, Default)]
#[repr(i32)]
pub enum UnknownPolicy {
    /// Follow each scope's allow_unknown setting.
    #[default]
    BySchema = 0,
    /// Accept unknown tags in every scope.
    Allow = 1,
    /// Reject unknown tags in every scope.
    Reject = 2,
}

/// Expected and observed values of one bounded constraint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaBounds {
    /// Inclusive minimum.
    pub minimum: usize,
    /// Inclusive maximum; usize::MAX means unrestricted.
    pub maximum: usize,
    /// Observed value at the violation.
    pub actual: usize,
}

/// Owned detailed C Schema violation; paths contain enclosing scopes only.
#[derive(Clone, Debug)]
pub struct SchemaDiagnostic {
    /// Owned subsystem context entries.
    pub contexts: Vec<crate::DiagnosticContext>,
    /// C error corresponding to the violation.
    pub error: Error,
    /// C severity code.
    pub severity: crate::Severity,
    /// Canonical violation kind.
    pub kind: crate::SchemaIssue,
    /// Name returned by the C kind helper.
    pub kind_name: String,
    /// Primary evidence domain, kind and coordinates.
    pub location: crate::Location,
    /// Native definition coordinate category (zero when absent).
    pub definition_kind: crate::SchemaDefinitionKind,
    /// Rule, group, type or component index in the supplied schema.
    pub definition_index: usize,
    /// Affected tag, separate from the enclosing path.
    pub tag: Tag,
    /// Retained outermost enclosing scope tags; None means no path evidence,
    /// distinct from a tracked empty path at the top level.
    pub path: Option<Vec<Tag>>,
    /// Number of innermost enclosing scopes omitted from `path`.
    pub path_omitted: usize,
    /// Affected source offset, when available.
    pub offset: Option<usize>,
    /// Owned native description of the expected condition.
    pub expected: Option<String>,
    /// Owned native description of the actual condition, when supplied.
    pub actual: Option<String>,
    /// Owned schema field or group name.
    pub field: Option<String>,
    /// Whether occurrence bounds describe an alternative group.
    pub is_group: bool,
    /// Expected and actual occurrences, when applicable.
    pub occurrences: Option<SchemaBounds>,
    /// Expected and actual lengths, when applicable.
    pub length: Option<SchemaBounds>,
    /// Expected form and actual constructed classification, when applicable.
    pub form: Option<(Kind, bool)>,
    /// Required length multiple; meaningful when length is present.
    pub length_multiple: usize,
    /// C length constraint flags; meaningful when length is present.
    pub length_flags: u32,
}

impl SchemaDiagnostic {
    // SAFETY: the caller keeps all native diagnostic spans and strings alive.
    pub(crate) unsafe fn from_raw(item: &native::tlv_schema_diagnostic_t) -> crate::Result<Self> {
        let detail = schema_detail(item);
        let path = if item.diagnostic.has_path != 0 {
            let mut path = Vec::with_capacity(item.diagnostic.path.length);
            for tag in &item.diagnostic.path.tags[..item.diagnostic.path.length] {
                // SAFETY: scope Tags borrow live input or schema storage.
                path.push(unsafe { Tag::from_raw(tag) }?);
            }
            Some(path)
        } else {
            None
        };
        // SAFETY: C returns a static NUL-terminated name for any kind.
        let kind_name =
            unsafe { std::ffi::CStr::from_ptr(native::tlv_schema_issue_kind_string(detail.kind)) }
                .to_string_lossy()
                .into_owned();
        let field = if detail.field.is_null() {
            None
        } else {
            // SAFETY: schema retains this NUL-terminated name during the copy.
            Some(
                unsafe { std::ffi::CStr::from_ptr(detail.field) }
                    .to_string_lossy()
                    .into_owned(),
            )
        };
        Ok(Self {
            error: Error::from_code(item.diagnostic.code).unwrap_or(Error::Schema),
            contexts: unsafe { crate::reader_diagnostic::contexts(&item.diagnostic) },
            severity: crate::Severity::from_raw(item.diagnostic.severity),
            kind: crate::SchemaIssue::from_raw(detail.kind),
            kind_name,
            location: crate::Location::from_raw(item.diagnostic.location),
            definition_kind: crate::SchemaDefinitionKind::from_raw(detail.definition.kind),
            definition_index: detail.definition.index,
            // SAFETY: affected tag borrows still-live storage.
            tag: unsafe { Tag::from_raw(&detail.tag) }?,
            path,
            path_omitted: item.diagnostic.path.omitted,
            offset: (item.diagnostic.location.kind != 0).then_some(item.diagnostic.location.begin),
            expected: if item.diagnostic.expected.is_null() {
                None
            } else {
                Some(
                    unsafe { std::ffi::CStr::from_ptr(item.diagnostic.expected) }
                        .to_string_lossy()
                        .into_owned(),
                )
            },
            actual: if item.diagnostic.actual.is_null() {
                None
            } else {
                Some(
                    unsafe { std::ffi::CStr::from_ptr(item.diagnostic.actual) }
                        .to_string_lossy()
                        .into_owned(),
                )
            },
            field,
            is_group: detail.is_group != 0,
            occurrences: (detail.has_occurs != 0).then_some(SchemaBounds {
                minimum: detail.min_occurs,
                maximum: detail.max_occurs,
                actual: detail.occurs,
            }),
            length: (detail.has_length != 0).then_some(SchemaBounds {
                minimum: detail.min_length,
                maximum: detail.max_length,
                actual: detail.actual_length,
            }),
            form: (detail.has_form != 0).then_some((
                match detail.expected_form {
                    1 => Kind::Primitive,
                    2 => Kind::Constructed,
                    _ => Kind::Any,
                },
                detail.actual_constructed != 0,
            )),
            length_multiple: detail.length_multiple,
            length_flags: detail.length_flags,
        })
    }
}

/// Detailed bounded report, independent of input and schema lifetimes.
#[derive(Clone, Debug)]
pub struct SchemaDiagnosticReport {
    /// Total violations, including those beyond storage capacity.
    pub total_count: usize,
    /// Stored prefix of detailed violations.
    pub diagnostics: Vec<SchemaDiagnostic>,
}

/// An alternative group whose total occurrences are checked by C.
#[derive(Clone, Debug)]
pub struct StructureGroup {
    /// Nonzero group identity, referenced by StructureRule::group.
    pub id: u32,
    /// Minimum occurrences across all member tags.
    pub min_occurs: usize,
    /// Maximum occurrences across all member tags.
    pub max_occurs: usize,
    /// Optional diagnostic group name; owned for the lifetime of the schema.
    pub name: Option<std::ffi::CString>,
}

impl StructureGroup {
    /// Create an unnamed group with inclusive occurrence bounds.
    pub fn new(id: u32, min_occurs: usize, max_occurs: usize) -> Self {
        Self {
            id,
            min_occurs,
            max_occurs,
            name: None,
        }
    }
    /// Set the diagnostic group name, rejecting embedded NUL bytes.
    pub fn named(mut self, name: &str) -> std::result::Result<Self, std::ffi::NulError> {
        self.name = Some(std::ffi::CString::new(name)?);
        Ok(self)
    }
}

// SAFETY: the tables are immutable after construction and only point to heap
// memory owned by the schema or to an immutable static.
unsafe impl Send for StructureSchema {}
unsafe impl Sync for StructureSchema {}

impl fmt::Debug for StructureSchema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StructureSchema")
            .field("rules", &self.as_raw().count)
            .finish_non_exhaustive()
    }
}

impl StructureSchema {
    /// Collect detailed expected/actual violations through the C validator.
    /// Zero capacity counts only; malformed input and invalid configuration return Err.
    pub fn validate_diagnostics<'f>(
        &self,
        data: &[u8],
        format: impl Into<crate::FormatRef<'f>>,
        limits: &ValidationLimits,
        unknown: UnknownPolicy,
        capacity: usize,
    ) -> std::result::Result<SchemaDiagnosticReport, SchemaError> {
        self.validate_diagnostics_raw(data, format.into().raw(), limits, unknown, capacity, None)
    }

    /// Same validation using a borrowed configurable Fixed Format.
    /// Equivalent to [`StructureSchema::validate_diagnostics`].
    pub fn validate_diagnostics_fixed(
        &self,
        data: &[u8],
        format: &crate::FixedFormat<'_>,
        limits: &ValidationLimits,
        unknown: UnknownPolicy,
        capacity: usize,
    ) -> std::result::Result<SchemaDiagnosticReport, SchemaError> {
        self.validate_diagnostics(data, format, limits, unknown, capacity)
    }

    fn validate_diagnostics_raw(
        &self,
        data: &[u8],
        format: *const native::tlv_format_t,
        limits: &ValidationLimits,
        unknown: UnknownPolicy,
        capacity: usize,
        checked: Option<&native::tlv_schema_checked_t>,
    ) -> std::result::Result<SchemaDiagnosticReport, SchemaError> {
        let convert = |error| SchemaError {
            error,
            offset: None,
            kind: crate::SchemaIssue::None,
            location: crate::Location::default(),
            definition_kind: crate::SchemaDefinitionKind::Unknown,
            definition_index: 0,
        };
        let mut storage = Vec::<std::mem::MaybeUninit<native::tlv_schema_diagnostic_t>>::new();
        storage
            .try_reserve_exact(capacity)
            .map_err(|_| convert(Error::OutOfMemory))?;
        storage.resize_with(capacity, std::mem::MaybeUninit::uninit);
        let mut report = native::tlv_schema_diagnostic_report_t {
            diagnostics: storage.as_mut_ptr().cast(),
            capacity,
            count: 0,
        };
        let mut diagnostic = new_diagnostic();
        // SAFETY: schema/input are live and C receives exclusive, correctly sized storage;
        // a checked handle borrows this schema.
        let code = unsafe {
            match checked {
                Some(checked) => native::tlv_schema_validate_all_checked(
                    checked,
                    data.as_ptr(),
                    data.len(),
                    format,
                    limits.max_depth,
                    limits.max_elements,
                    unknown as i32,
                    &mut report,
                    &mut diagnostic,
                ),
                None => native::tlv_schema_validate_all_diag(
                    data.as_ptr(),
                    data.len(),
                    format,
                    self.as_raw(),
                    limits.max_depth,
                    limits.max_elements,
                    unknown as i32,
                    &mut report,
                    &mut diagnostic,
                ),
            }
        };
        if code != native::TLV_OK && code != native::TLV_ERR_SCHEMA {
            return Err(SchemaError::from_raw(
                Error::from_code(code).unwrap(),
                &diagnostic,
            ));
        }
        let mut diagnostics = Vec::new();
        for item in storage.iter().take(report.count.min(capacity)) {
            // SAFETY: C initialized the stored prefix; its borrowed data remains live here.
            let item = unsafe { item.assume_init_ref() };
            // SAFETY: C initialized this record and input/schema storage is live.
            diagnostics.push(unsafe { SchemaDiagnostic::from_raw(item) }.map_err(convert)?);
        }
        Ok(SchemaDiagnosticReport {
            total_count: report.count,
            diagnostics,
        })
    }

    /// Creates a schema from `rules`. Tags must be unique; a duplicate tag is
    /// reported by [`StructureSchema::validate`] as [`Error::Schema`]. Unknown
    /// tags are rejected unless `allow_unknown` is `true`.
    pub fn new(
        rules: impl IntoIterator<Item = StructureRule>,
        allow_unknown: bool,
    ) -> StructureSchema {
        Self::with_constraints(rules, allow_unknown, SchemaOrder::Any, [])
    }

    /// Construct a schema with native ordering and alternative-group constraints.
    /// Invalid groups, duplicate identities and incompatible rules are rejected
    /// by the canonical validator when the schema is used.
    pub fn with_constraints(
        rules: impl IntoIterator<Item = StructureRule>,
        allow_unknown: bool,
        order: SchemaOrder,
        groups: impl IntoIterator<Item = StructureGroup>,
    ) -> Self {
        let mut raw_rules = Vec::new();
        let mut fields = Vec::new();
        let mut children = Vec::new();
        let mut tags = Vec::new();
        let mut names = Vec::new();
        for rule in rules {
            let child_ptr = match rule.children {
                Some(child) => {
                    let ptr = child.as_raw() as *const native::tlv_structure_schema_t;
                    children.push(child);
                    ptr
                }
                None => ptr::null(),
            };
            fields.push(native::tlv_schema_entry_t {
                tag: rule.tag.raw(),
                min_length: rule.min_length,
                max_length: rule.max_length,
                flags: rule.flags,
                name: rule.name.as_ref().map_or(ptr::null(), |name| name.as_ptr()),
                length_multiple: rule.length_multiple,
            });
            raw_rules.push(native::tlv_structure_rule_t {
                entry: ptr::null(),
                min_occurs: rule.min_occurs,
                max_occurs: rule.max_occurs,
                kind: rule.kind.raw(),
                children: child_ptr,
                group: rule.group,
            });
            // Moving a tag moves its handle, not the heap bytes the rule borrows.
            tags.push(rule.tag);
            if let Some(name) = rule.name {
                names.push(name);
            }
        }
        for (rule, field) in raw_rules.iter_mut().zip(fields.iter()) {
            rule.entry = field;
        }
        let groups: Vec<_> = groups
            .into_iter()
            .map(|group| {
                let name = group
                    .name
                    .as_ref()
                    .map_or(ptr::null(), |name| name.as_ptr());
                if let Some(value) = group.name {
                    names.push(value);
                }
                native::tlv_structure_group_t {
                    id: group.id,
                    min_occurs: group.min_occurs,
                    max_occurs: group.max_occurs,
                    name,
                }
            })
            .collect();
        let mut compiled = Box::new(Compiled {
            raw: native::tlv_structure_schema_t {
                rules: ptr::null(),
                count: raw_rules.len(),
                allow_unknown: allow_unknown as i32,
                groups: groups.as_ptr(),
                group_count: groups.len(),
                order: match order {
                    SchemaOrder::Any => native::TLV_SCHEMA_ORDER_ANY,
                    SchemaOrder::Sequence => native::TLV_SCHEMA_ORDER_SEQUENCE,
                },
            },
            rules: raw_rules,
            _fields: fields,
            _tags: tags,
            _children: children,
            _groups: groups,
            _names: names,
        });
        // The vector is never resized again, so its buffer address is stable.
        compiled.raw.rules = compiled.rules.as_ptr();
        StructureSchema {
            backing: Backing::Owned(compiled),
        }
    }

    /// Returns the built-in structural schema of common EMV Book 3 top-level
    /// data objects (FCI, application template, GPO response format 2).
    ///
    /// Validate with [`Format::Ber`](crate::Format::Ber).
    pub fn emv() -> StructureSchema {
        // SAFETY: only the address of an immutable static is taken; it lives
        // for the whole program and is never written.
        let raw = unsafe { &*ptr::addr_of!(native::tlv_emv_structure_schema) };
        StructureSchema {
            backing: Backing::Static(raw),
        }
    }

    fn as_raw(&self) -> &native::tlv_structure_schema_t {
        match &self.backing {
            Backing::Owned(compiled) => &compiled.raw,
            Backing::Static(raw) => raw,
        }
    }

    /// Validates framing, nesting, lengths, occurrence counts and child
    /// membership of `data` against the schema.
    ///
    /// Values are never decoded. Which tags are constructed is decided by
    /// `format`: BER, CER and DER nest by their constructed bit, and preserve the C format
    /// classification of each element.
    ///
    /// # Errors
    ///
    /// A [`SchemaError`] carrying the C library's error and the offset of the
    /// failure: [`Error::Schema`] with missing detail for an absent required field,
    /// [`Error::Schema`] for other rule violations including length constraints,
    /// [`Error::InvalidSchema`] for a malformed definition, or a reader error
    /// such as [`Error::Limit`].
    pub fn validate<'f>(
        &self,
        data: &[u8],
        format: impl Into<crate::FormatRef<'f>>,
        limits: &ValidationLimits,
    ) -> std::result::Result<(), SchemaError> {
        self.validate_raw(data, format.into().raw(), limits, None)
    }

    /// Same validation using a borrowed configurable Fixed Format.
    /// Equivalent to [`StructureSchema::validate`].
    pub fn validate_fixed(
        &self,
        data: &[u8],
        format: &crate::FixedFormat<'_>,
        limits: &ValidationLimits,
    ) -> std::result::Result<(), SchemaError> {
        self.validate(data, format, limits)
    }

    fn validate_raw(
        &self,
        data: &[u8],
        format: *const native::tlv_format_t,
        limits: &ValidationLimits,
        checked: Option<&native::tlv_schema_checked_t>,
    ) -> std::result::Result<(), SchemaError> {
        let mut diagnostic = new_diagnostic();
        // SAFETY: `data` is a valid slice; the format and schema tables are
        // valid for the call and immutable; `diagnostic` is writable native storage.
        // A checked handle borrows this schema.
        let code = unsafe {
            match checked {
                Some(checked) => native::tlv_schema_validate_checked(
                    checked,
                    data.as_ptr(),
                    data.len(),
                    format,
                    limits.max_depth,
                    limits.max_elements,
                    &mut diagnostic,
                ),
                None => native::tlv_schema_validate(
                    data.as_ptr(),
                    data.len(),
                    format,
                    self.as_raw(),
                    limits.max_depth,
                    limits.max_elements,
                    &mut diagnostic,
                ),
            }
        };
        match Error::from_code(code) {
            None => Ok(()),
            Some(error) => Err(SchemaError::from_raw(error, &diagnostic)),
        }
    }

    /// Checks every reachable rule table of the definition through
    /// `tlv_schema_check()`, without input. Shared and recursive child schemas
    /// are checked once each.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidSchema`] with the failing rule or group as
    /// `definition_kind` and `definition_index`, or [`Error::Unsupported`] when
    /// the graph exceeds Core's bounded checking capacity.
    pub fn check_definition(&self) -> std::result::Result<(), SchemaError> {
        let mut diagnostic = new_diagnostic();
        // SAFETY: immutable live tables and writable diagnostic.
        let code = unsafe { native::tlv_schema_check(self.as_raw(), &mut diagnostic) };
        match Error::from_code(code) {
            None => Ok(()),
            Some(error) => Err(SchemaError::from_raw(error, &diagnostic)),
        }
    }

    /// Checks the definition once through `tlv_schema_prepare()` and returns a
    /// handle that validates repeatedly without checking it again. The handle
    /// borrows this schema, whose tables cannot change.
    ///
    /// # Errors
    ///
    /// The same as [`StructureSchema::check_definition`].
    pub fn check(&self) -> std::result::Result<CheckedStructureSchema<'_>, SchemaError> {
        let mut raw = native::tlv_schema_checked_t {
            schema: ptr::null(),
        };
        let mut diagnostic = new_diagnostic();
        // SAFETY: the handle borrows these immutable tables for the schema's lifetime.
        let code = unsafe { native::tlv_schema_prepare(&mut raw, self.as_raw(), &mut diagnostic) };
        match Error::from_code(code) {
            None => Ok(CheckedStructureSchema { schema: self, raw }),
            Some(error) => Err(SchemaError::from_raw(error, &diagnostic)),
        }
    }
}

/// A [`StructureSchema`] whose definition was checked once by
/// [`StructureSchema::check`]. Validation behaves like the schema's own, but
/// never repeats the definition check and so never fails with
/// [`Error::InvalidSchema`].
///
/// ```
/// use opentlv::{Format, StructureRule, StructureSchema, Tag, ValidationLimits};
///
/// let rule = StructureRule::new(Tag::from_bytes(&[1])).required_once();
/// let schema = StructureSchema::new([rule], false);
/// let checked = schema.check()?;
/// let limits = ValidationLimits::default();
/// for input in [&[1, 0][..], &[1, 1, 7]] {
///     checked.validate(input, Format::Ber, &limits)?;
/// }
/// # Ok::<(), opentlv::SchemaError>(())
/// ```
#[derive(Clone, Copy)]
pub struct CheckedStructureSchema<'s> {
    schema: &'s StructureSchema,
    raw: native::tlv_schema_checked_t,
}

// SAFETY: the handle only borrows the immutable, Send + Sync schema tables.
unsafe impl Send for CheckedStructureSchema<'_> {}
unsafe impl Sync for CheckedStructureSchema<'_> {}

impl fmt::Debug for CheckedStructureSchema<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CheckedStructureSchema")
            .field("schema", self.schema)
            .finish()
    }
}

impl<'s> CheckedStructureSchema<'s> {
    /// The checked schema.
    pub fn schema(&self) -> &'s StructureSchema {
        self.schema
    }

    /// Validates like [`StructureSchema::validate`], through
    /// `tlv_schema_validate_checked()`.
    pub fn validate<'f>(
        &self,
        data: &[u8],
        format: impl Into<crate::FormatRef<'f>>,
        limits: &ValidationLimits,
    ) -> std::result::Result<(), SchemaError> {
        self.schema
            .validate_raw(data, format.into().raw(), limits, Some(&self.raw))
    }

    /// Reports violations like [`StructureSchema::validate_diagnostics`],
    /// through `tlv_schema_validate_all_checked()`.
    pub fn validate_diagnostics<'f>(
        &self,
        data: &[u8],
        format: impl Into<crate::FormatRef<'f>>,
        limits: &ValidationLimits,
        unknown: UnknownPolicy,
        capacity: usize,
    ) -> std::result::Result<SchemaDiagnosticReport, SchemaError> {
        self.schema.validate_diagnostics_raw(
            data,
            format.into().raw(),
            limits,
            unknown,
            capacity,
            Some(&self.raw),
        )
    }
}

/// A Schema diagnostic reset by C to all-unset.
fn new_diagnostic() -> native::tlv_schema_diagnostic_t {
    let mut diagnostic = std::mem::MaybeUninit::uninit();
    // SAFETY: C writes every byte of the writable diagnostic.
    unsafe {
        native::tlv_schema_diagnostic_init(diagnostic.as_mut_ptr());
        diagnostic.assume_init()
    }
}

/// Resource limits for [`StructureSchema::validate`].
///
/// Both limits are inclusive and zero is a real limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidationLimits {
    /// Maximum nesting depth; the C library accepts at most 64.
    pub max_depth: usize,
    /// Maximum total number of elements.
    pub max_elements: usize,
}

impl Default for ValidationLimits {
    /// A depth of 32 and 100 000 elements, the defaults of the DER and CER
    /// standards.
    fn default() -> ValidationLimits {
        ValidationLimits {
            max_depth: 32,
            max_elements: 100_000,
        }
    }
}

/// A failed [`StructureSchema::validate`]: the error and where it happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchemaError {
    /// The error reported by the C library.
    pub error: Error,
    /// Known byte position; use location.kind to distinguish scope end from a point.
    /// Definition errors have no input position.
    pub offset: Option<usize>,
    /// Canonical Schema reason.
    pub kind: crate::SchemaIssue,
    /// Canonical primary evidence location.
    pub location: crate::Location,
    /// Native definition coordinate category (zero when absent).
    pub definition_kind: crate::SchemaDefinitionKind,
    /// Rule, group, type or component index in the supplied schema.
    pub definition_index: usize,
}

impl SchemaError {
    fn from_raw(error: Error, diagnostic: &native::tlv_schema_diagnostic_t) -> Self {
        let detail = schema_detail(diagnostic);
        let location = crate::Location::from_raw(diagnostic.diagnostic.location);
        SchemaError {
            error,
            offset: location.offset(),
            kind: crate::SchemaIssue::from_raw(detail.kind),
            location,
            definition_kind: crate::SchemaDefinitionKind::from_raw(detail.definition.kind),
            definition_index: detail.definition.index,
        }
    }
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.offset {
            Some(offset) => write!(f, "{} at offset {}", self.error, offset),
            None => write!(f, "{}", self.error),
        }
    }
}

impl error::Error for SchemaError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(&self.error)
    }
}

impl From<SchemaError> for Error {
    fn from(err: SchemaError) -> Error {
        err.error
    }
}

/// Schema detail of a native diagnostic; other causes carry no Schema finding.
pub(crate) fn schema_detail(item: &native::tlv_schema_diagnostic_t) -> native::tlv_schema_detail_t {
    if item.cause == native::TLV_SCHEMA_CAUSE_SCHEMA {
        // SAFETY: cause selects the initialized Schema member.
        unsafe { item.detail.schema }
    } else {
        // SAFETY: the all-zero Schema detail has valid enums and NULL pointers.
        unsafe { std::mem::zeroed() }
    }
}
