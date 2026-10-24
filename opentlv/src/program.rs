// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel
//! Immutable compiled C Query programs with independent bounded continuations.
use crate::format::FormatOwner;
use crate::{Element, Error, Format, ReaderDiagnostic, TreeReader, Visit};
use opentlv_sys as native;
mod providers;
mod schema;
pub use providers::{QueryDefinitionScope, QueryResolver, QueryTagAdapter};
pub use schema::{QueryRule, QuerySchema, QuerySchemaDetail, QuerySchemaError, QuerySchemaLimits};
use std::{
    collections::BTreeMap,
    ffi::{CStr, CString},
    marker::PhantomData,
    mem::{size_of, zeroed},
    os::raw::c_void,
    panic::{catch_unwind, AssertUnwindSafe},
    ptr, slice,
    sync::Arc,
};

/// Typed expression/binding category. Boolean bindings are not part of this language version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum QueryType {
    /// Ordered unique node sequence.
    Nodes = 0,
    /// Boolean scalar.
    Boolean = 1,
    /// Signed 64-bit integer.
    Integer = 2,
    /// Byte span.
    Bytes = 3,
    /// Validated UTF-8 string.
    String = 4,
}
/// One closed C conversion selector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum QueryConversion {
    /// Domain integer conversion.
    Num = 0,
    /// Domain packed-decimal conversion.
    Bcd = 1,
    /// Domain UTF-8 conversion.
    Text = 2,
    /// Domain UTC Unix-seconds conversion.
    Date = 3,
}
/// Owned callback output; text is copied to bounded native frame scratch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueryDecoded {
    /// Signed 64-bit integer.
    Integer(i64),
    /// Validated UTF-8 string.
    String(String),
}
/// Optional complete-node metadata copied for a conversion callback.
#[derive(Clone, Debug)]
pub struct QueryMetadata {
    /// Owned raw candidate Tag bytes.
    pub tag: Vec<u8>,
    /// Owned complete candidate Value bytes, independent of conversion input.
    pub value: Vec<u8>,
    /// Canonical BEGIN/ELEMENT kind.
    pub kind: i32,
    /// Node depth.
    pub depth: usize,
    /// Original event byte offset, when supplied by the producer.
    pub offset: usize,
}
type DecodeProvider =
    dyn Fn(&[u8], Option<QueryMetadata>) -> std::result::Result<QueryDecoded, Error> + Send + Sync;
#[derive(Clone)]
enum Decoder {
    Callback(Arc<DecodeProvider>),
    Codec(Arc<CodecAdapter>),
}
/// A number codec composed for `tlv_query_codec_decode()`; its pointers refer
/// into this allocation, which never moves once shared.
struct CodecAdapter {
    config: native::tlv_number_codec_config_t,
    codec: native::tlv_codec_t,
    adapter: native::tlv_query_codec_adapter_t,
}
// SAFETY: immutable after construction; the pointers refer only to its own fields
// and to the immutable C number codec functions.
unsafe impl Send for CodecAdapter {}
unsafe impl Sync for CodecAdapter {}
/// Owning thread-safe provider. Stable ID and scratch capacity enter the C image;
/// callback code remains outside the engine's allocation and work contracts.
#[derive(Clone)]
pub struct QueryProvider {
    /// Closed conversion selector replaced by this provider.
    pub conversion: QueryConversion,
    /// Nonzero caller-assigned uint32 compatibility ID.
    pub id: u32,
    /// Maximum output bytes in each native frame: UTF-8 text of a callback, or
    /// the decoded `u64` of a [`QueryProvider::number`] codec.
    pub max_result_bytes: usize,
    decode: Decoder,
}
impl std::fmt::Debug for QueryProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryProvider")
            .field("conversion", &self.conversion)
            .field("id", &self.id)
            .field("max_result_bytes", &self.max_result_bytes)
            .finish_non_exhaustive()
    }
}
impl QueryProvider {
    /// Retain a decoder for all programs/executions using it. Panics are contained
    /// at the FFI boundary and reported as native Callback diagnostics.
    pub fn new<F>(conversion: QueryConversion, id: u32, max_result_bytes: usize, decode: F) -> Self
    where
        F: Fn(&[u8], Option<QueryMetadata>) -> std::result::Result<QueryDecoded, Error>
            + Send
            + Sync
            + 'static,
    {
        Self {
            conversion,
            id,
            max_result_bytes,
            decode: Decoder::Callback(Arc::new(decode)),
        }
    }
    /// Decode Values of an integer conversion (`Num`, `Bcd` or `Date`) with
    /// Core's [`NumberCodec`](crate::NumberCodec) through
    /// `tlv_query_codec_decode()`, without a Rust callback. Codec failures keep
    /// their native detail; a decoded value above `i64::MAX` fails with
    /// [`Error::InvalidValue`].
    pub fn number(conversion: QueryConversion, id: u32, codec: crate::NumberCodec) -> Self {
        let mut owner = Arc::new(CodecAdapter {
            config: codec.config(),
            codec: native::tlv_codec_t {
                context: ptr::null(),
                decode: Some(native::tlv_number_decode),
                encode: Some(native::tlv_number_encode),
            },
            adapter: native::tlv_query_codec_adapter_t {
                codec: ptr::null(),
                representation: native::TLV_QUERY_CODEC_UNSIGNED,
            },
        });
        let inner = Arc::get_mut(&mut owner).expect("a new Arc is unique");
        inner.codec.context = ptr::addr_of!(inner.config).cast();
        inner.adapter.codec = ptr::addr_of!(inner.codec);
        Self {
            conversion,
            id,
            max_result_bytes: size_of::<u64>(),
            decode: Decoder::Codec(owner),
        }
    }
}
unsafe extern "C" fn decode_provider(
    context: *const c_void,
    event: *const native::tlv_tree_event_t,
    data: *const u8,
    size: usize,
    scratch: *mut c_void,
    capacity: usize,
    result: *mut native::tlv_query_result_t,
    _diagnostic: *mut native::tlv_codec_diagnostic_t,
) -> i32 {
    // SAFETY: the program owns its boxed providers; C lends complete bounded
    // input and exclusive frame scratch for this synchronous invocation.
    let provider = unsafe { &*context.cast::<QueryProvider>() };
    let Decoder::Callback(decode) = &provider.decode else {
        return Error::InvalidState.code();
    };
    let input = if size == 0 {
        &[]
    } else {
        unsafe { slice::from_raw_parts(data, size) }
    };
    let metadata = if event.is_null() {
        None
    } else {
        let event = unsafe { &*event };
        let Ok(value_size) = usize::try_from(event.element.value.size) else {
            return Error::InvalidValue.code();
        };
        Some(QueryMetadata {
            tag: if event.element.tag.size == 0 {
                Vec::new()
            } else {
                unsafe {
                    slice::from_raw_parts(event.element.tag.data, event.element.tag.size).to_vec()
                }
            },
            value: if event.element.value.size == 0 {
                Vec::new()
            } else {
                unsafe { slice::from_raw_parts(event.element.value.data, value_size).to_vec() }
            },
            kind: event.kind,
            depth: event.depth,
            offset: event.offset,
        })
    };
    match catch_unwind(AssertUnwindSafe(|| decode(input, metadata))) {
        Ok(Ok(value)) => {
            let mut output: native::tlv_query_result_t = unsafe { zeroed() };
            match value {
                QueryDecoded::Integer(integer) => {
                    output.kind = 2;
                    output.integer = integer;
                }
                QueryDecoded::String(text) => {
                    if text.len() > capacity {
                        return Error::BufferTooShort.code();
                    }
                    if !text.is_empty() {
                        unsafe {
                            ptr::copy_nonoverlapping(text.as_ptr(), scratch.cast(), text.len())
                        };
                    }
                    output.kind = 4;
                    output.data = scratch.cast();
                    output.size = text.len();
                }
            }
            unsafe {
                *result = output;
            }
            0
        }
        Ok(Err(code)) => code.code(),
        Err(_) => Error::Callback.code(),
    }
}
/// Exhausted Query resource reported by a LIMIT failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryLimit {
    /// Owned immutable name of the exhausted resource.
    pub name: Box<str>,
    /// Configured bound for the named resource.
    pub configured: usize,
}

/// Complete owned native failure, independent of input/program lifetime.
#[derive(Clone, Debug)]
pub struct ProgramError {
    /// Owned common evidence when no Reader cause carries it.
    ///
    /// None for Reader failures, whose `reader` holds the same evidence, and when C
    /// reported only error severity without text, contexts or path. Use the accessors
    /// below to read either source.
    pub metadata: Option<Box<crate::DiagnosticMetadata>>,
    /// Primary evidence: the input or Value location of the failing node or lower
    /// layer, otherwise the expression span, or unknown for admission and lifecycle
    /// failures.
    pub location: crate::Location,
    /// Original native status, including resumable NEED_MORE_DATA.
    pub error: Error,
    /// Native Query diagnostic category.
    pub kind: crate::QueryErrorKind,
    /// Related expression span when another known location is primary; otherwise
    /// unknown, so the span `location` already holds is not repeated.
    pub expression: crate::Location,
    /// Exhausted named resource and its configured bound, when present.
    ///
    /// Boxed with the rare limit evidence so `location` stays inline.
    pub limit: Option<Box<QueryLimit>>,
    /// Owned conversion cause supplied by the provider; `reported` holds its raw result.
    pub codec_detail: Option<Box<crate::CodecDiagnostic>>,
    /// Owned original Reader diagnostic for Reader failures.
    pub reader: Option<Box<ReaderDiagnostic>>,
}
impl ProgramError {
    /// Canonical severity of the common diagnostic.
    pub fn severity(&self) -> crate::Severity {
        match (&self.reader, &self.metadata) {
            (Some(reader), _) => reader.severity,
            (None, Some(metadata)) => metadata.severity,
            (None, None) => crate::Severity::Error,
        }
    }
    /// Owned subsystem contexts of the common diagnostic.
    pub fn contexts(&self) -> &[crate::DiagnosticContext] {
        match (&self.reader, &self.metadata) {
            (Some(reader), _) => &reader.contexts,
            (None, Some(metadata)) => &metadata.contexts,
            (None, None) => &[],
        }
    }
    /// Retained enclosing tags; None means no path evidence.
    pub fn path(&self) -> Option<&[Vec<u8>]> {
        match (&self.reader, &self.metadata) {
            (Some(reader), _) => reader.path.as_deref(),
            (None, Some(metadata)) => metadata.path.as_deref(),
            (None, None) => None,
        }
    }
    /// Native expected-token or type description of the common diagnostic.
    pub fn expected(&self) -> Option<&str> {
        match (&self.reader, &self.metadata) {
            (Some(reader), _) => reader.expected.as_deref(),
            (None, Some(metadata)) => metadata.expected.as_deref(),
            (None, None) => None,
        }
    }
    /// Native actual-value description of the common diagnostic.
    pub fn actual(&self) -> Option<&str> {
        match (&self.reader, &self.metadata) {
            (Some(reader), _) => reader.actual.as_deref(),
            (None, Some(metadata)) => metadata.actual.as_deref(),
            (None, None) => None,
        }
    }
    /// `path()` as Core formats it, such as `"6F > A5"`; see
    /// [`crate::DiagnosticMetadata::path_text`].
    pub fn path_text(&self) -> Option<String> {
        crate::reader_diagnostic::path_text(self.path(), self.path_omitted())
    }
    /// Number of omitted innermost path scopes.
    pub fn path_omitted(&self) -> usize {
        match (&self.reader, &self.metadata) {
            (Some(reader), _) => reader.path_omitted,
            (None, Some(metadata)) => metadata.path_omitted,
            (None, None) => 0,
        }
    }
    /// Known primary input offset, derived from `location` without duplicate storage.
    pub fn source_offset(&self) -> Option<usize> {
        (self.location.domain == crate::LocationDomain::Input)
            .then(|| self.location.offset())
            .flatten()
    }
}
impl std::fmt::Display for ProgramError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({}) at {}", self.error, self.kind, self.location)
    }
}
impl std::error::Error for ProgramError {}
/// Result retaining complete owned Query failure context.
pub type ProgramResult<T> = std::result::Result<T, ProgramError>;
pub(crate) fn check(code: i32, diagnostic: &native::tlv_query_diagnostic_t) -> ProgramResult<()> {
    Error::check(code).map_err(|error| {
        // SAFETY: diagnostics come from C; descriptions are static NUL-terminated strings.
        unsafe {
            let text = |p: *const std::os::raw::c_char| {
                (!p.is_null()).then(|| {
                    CStr::from_ptr(p)
                        .to_string_lossy()
                        .into_owned()
                        .into_boxed_str()
                })
            };
            let common = &diagnostic.diagnostic;
            // C initializes the whole union, so copying the inactive arm reads defined bytes.
            let codec = (diagnostic.cause == native::TLV_QUERY_CAUSE_CODEC)
                .then_some(diagnostic.detail.codec);
            // Reader causes own the common evidence; plain failures skip the allocation.
            let reported = !common.expected.is_null()
                || !common.actual.is_null()
                || !common.contexts.is_null()
                || common.has_path != 0
                || crate::Severity::from_raw(common.severity) != crate::Severity::Error;
            ProgramError {
                location: crate::Location::from_raw(common.location),
                metadata: (diagnostic.cause != native::TLV_QUERY_CAUSE_READER && reported).then(
                    || {
                        Box::new(crate::DiagnosticMetadata {
                            expected: text(common.expected),
                            actual: text(common.actual),
                            severity: crate::Severity::from_raw(common.severity),
                            contexts: crate::reader_diagnostic::contexts(common),
                            path: crate::reader_diagnostic::path(common),
                            path_omitted: common.path.omitted,
                        })
                    },
                ),
                error,
                kind: crate::QueryErrorKind::from_raw(if code == native::TLV_ERR_INVALID_STATE {
                    native::TLV_QUERY_ERROR_STATE
                } else {
                    diagnostic.kind
                }),
                expression: crate::Location::from_raw(diagnostic.expression),
                limit: text(diagnostic.limit).map(|name| {
                    Box::new(QueryLimit {
                        name,
                        configured: diagnostic.configured,
                    })
                }),
                codec_detail: codec.map(|codec| {
                    Box::new(crate::CodecDiagnostic::from_raw(
                        &native::tlv_codec_diagnostic_t {
                            diagnostic: diagnostic.diagnostic,
                            codec,
                        },
                    ))
                }),
                reader: (diagnostic.cause == native::TLV_QUERY_CAUSE_READER).then(|| {
                    Box::new(ReaderDiagnostic::from_parts(
                        &diagnostic.diagnostic,
                        &diagnostic.detail.reader,
                    ))
                }),
            }
        }
    })
}
fn plain(code: i32) -> ProgramResult<()> {
    // SAFETY: all-zero diagnostic contains valid enums, NULL pointers and zero counts.
    check(code, &unsafe { zeroed() })
}
#[repr(align(16))]
#[derive(Clone)]
struct Block([u8; 16]);
struct Memory {
    blocks: Vec<Block>,
    bytes: usize,
}
impl Memory {
    fn new(bytes: usize) -> ProgramResult<Self> {
        let count = bytes
            .checked_add(15)
            .ok_or_else(|| plain(native::TLV_ERR_OVERFLOW).unwrap_err())?
            / 16;
        let mut blocks = Vec::new();
        blocks
            .try_reserve_exact(count.max(1))
            .map_err(|_| plain(native::TLV_ERR_OUT_OF_MEMORY).unwrap_err())?;
        blocks.resize(count.max(1), Block([0; 16]));
        Ok(Self { blocks, bytes })
    }
    fn data(&self) -> *const c_void {
        self.blocks[0].0.as_ptr().cast()
    }
    fn data_mut(&mut self) -> *mut c_void {
        self.blocks[0].0.as_mut_ptr().cast()
    }
}
/// Owning compiler configuration. Names are resolved only during compilation.
#[derive(Clone, Debug)]
pub struct ProgramOptions {
    /// Explicit immutable builtin Format and semantic capability selection.
    pub format: Format,
    /// Owned configurable Fixed descriptor; when present it replaces `format`.
    pub fixed_format: Option<crate::OwnedFixedFormat>,
    /// Shared composed descriptor; when present it replaces `format`. Setting
    /// both custom descriptors is rejected with `InvalidArg`.
    pub composed_format: Option<crate::ComposedFormat>,
    /// Declared typed variables; names omit the dollar prefix.
    pub variables: BTreeMap<String, QueryType>,
    /// Owned scoped symbolic spellings mapped to raw Tag bytes.
    pub names: BTreeMap<String, Vec<u8>>,
    /// Compile-only dynamic or Definition resolver, replacing the `names` map.
    pub resolver: Option<QueryResolver>,
    /// Optional semantic Tag callbacks, replacing the builtin ASN.1 adapter.
    pub tags: Option<QueryTagAdapter>,
    /// Owned custom conversion providers; duplicate selectors/IDs are rejected.
    pub providers: Vec<QueryProvider>,
    /// Enable the canonical C optimizer.
    pub optimize: bool,
    /// Maximum Query text bytes.
    pub max_text: usize,
    /// Maximum lexical tokens.
    pub max_tokens: usize,
    /// Maximum parser nesting.
    pub max_nesting: usize,
    /// Maximum expression instructions.
    pub max_states: usize,
    /// Runtime pattern capacity.
    pub max_pattern: usize,
    /// Maximum copied symbolic Tag bytes.
    pub max_resolved_tag: usize,
}
impl Default for ProgramOptions {
    fn default() -> Self {
        // SAFETY: C initializer writes every option field.
        let config = unsafe {
            let mut c = zeroed();
            native::tlv_query_compile_options_init(&mut c);
            c
        };
        Self {
            format: Format::Ber,
            fixed_format: None,
            composed_format: None,
            variables: BTreeMap::new(),
            names: BTreeMap::new(),
            resolver: None,
            tags: None,
            providers: Vec::new(),
            optimize: true,
            max_text: config.max_text,
            max_tokens: config.max_tokens,
            max_nesting: config.max_nesting,
            max_states: config.max_states,
            max_pattern: config.max_pattern,
            max_resolved_tag: config.max_resolved_tag,
        }
    }
}
unsafe extern "C" fn resolve(
    context: *const c_void,
    space: *const std::os::raw::c_char,
    space_size: usize,
    name: *const std::os::raw::c_char,
    name_size: usize,
    tag: *mut native::tlv_tag_t,
) -> i32 {
    // SAFETY: compile keeps the map and bounded parser spans alive; output is writable.
    let names = unsafe { &*context.cast::<BTreeMap<String, Vec<u8>>>() };
    let text = |p, n| {
        if n == 0 {
            Ok("")
        } else {
            unsafe { std::str::from_utf8(slice::from_raw_parts(p as *const u8, n)) }
        }
    };
    let (Ok(namespace), Ok(name)) = (text(space, space_size), text(name, name_size)) else {
        return native::TLV_ERR_INVALID_ARG;
    };
    let key = if namespace.is_empty() {
        name.to_owned()
    } else {
        format!("{namespace}:{name}")
    };
    match names.get(&key) {
        Some(value) => {
            unsafe {
                *tag = native::tlv_tag_t {
                    data: value.as_ptr(),
                    size: value.len(),
                };
            }
            native::TLV_OK
        }
        None => native::TLV_ERR_INVALID_TAG,
    }
}
/// Program input: Query text, a loaded image, or a plan opened without the frontend.
#[derive(Clone, Copy)]
enum Source {
    Text,
    Image,
    Plan,
}
struct ProgramStorage {
    memory: Memory,
    info: native::tlv_query_program_info_t,
    environment: native::tlv_query_environment_t,
    _hooks: Box<[native::tlv_query_hook_t]>,
    _providers: Box<[QueryProvider]>,
    _tags: Option<Box<providers::TagOwner>>,
    _format: FormatOwner,
}
// SAFETY: the image is immutable; providers are either immutable builtin C storage
// or owned Send + Sync Rust callbacks. Resolver contexts are compile-only.
unsafe impl Send for ProgramStorage {}
unsafe impl Sync for ProgramStorage {}
/// Immutable program; clones share storage, while each execution owns distinct workspace.
#[derive(Clone)]
pub struct QueryProgram {
    storage: Arc<ProgramStorage>,
}
/// Completed-selection edit; C resolves overlapping targets and prevalidates framing.
#[cfg(feature = "document")]
pub enum QueryEdit<'a> {
    /// Remove selected roots, with ancestor dominance.
    Remove,
    /// Replace complete Values, with ancestor dominance.
    Replace(&'a [u8]),
    /// Insert one sibling after every selected target.
    InsertAfter {
        /// Raw insertion Tag.
        tag: &'a [u8],
        /// Complete insertion Value.
        value: &'a [u8],
    },
}
/// Explicit bounded storage/work requirements for owning Document edits.
#[cfg(feature = "document")]
#[derive(Clone, Copy, Debug)]
pub struct QueryEditOptions {
    /// Maximum traversal depth.
    pub depth: usize,
    /// Retained node capacity, including nonmatches.
    pub nodes: usize,
    /// Native operation budget.
    pub work: usize,
    /// Target pointer entries; None uses nodes. Short capacity makes no changes.
    pub target_capacity: Option<usize>,
    /// Canonical Value snapshot bytes; None measures the Document.
    pub value_capacity: Option<usize>,
}
#[cfg(feature = "document")]
impl Default for QueryEditOptions {
    fn default() -> Self {
        Self {
            depth: 64,
            nodes: 1024,
            work: 100000000,
            target_capacity: None,
            value_capacity: None,
        }
    }
}
/// Edit failure with the count of preceding committed mutations.
#[cfg(feature = "document")]
#[derive(Debug)]
pub struct QueryEditError {
    /// Original native failure.
    pub failure: ProgramError,
    /// Committed selected roots; no rollback is implied.
    pub applied: usize,
}
#[cfg(feature = "document")]
impl std::fmt::Display for QueryEditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} after {} edits", self.failure, self.applied)
    }
}
#[cfg(feature = "document")]
impl std::error::Error for QueryEditError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.failure)
    }
}
impl QueryProgram {
    /// Select and edit exclusively borrowed Document storage through C. No Node
    /// borrow can coexist with the mutable Document argument. Short target capacity
    /// leaves the Document unchanged; retry this program with a larger bound.
    #[cfg(feature = "document")]
    pub fn edit_document(
        &self,
        document: &mut crate::Document<'_>,
        edit: QueryEdit<'_>,
        options: QueryEditOptions,
    ) -> std::result::Result<usize, QueryEditError> {
        let initial = |failure| QueryEditError {
            failure,
            applied: 0,
        };
        let raw = document.raw;
        let mut execution = self
            .execution(options.depth, options.nodes, options.work, true)
            .map_err(initial)?;
        execution
            .evaluate_document(document, None, options.value_capacity)
            .map_err(initial)?;
        let capacity = options.target_capacity.unwrap_or(options.nodes);
        let mut targets = Vec::<*mut native::tlv_node_t>::new();
        targets
            .try_reserve_exact(capacity)
            .map_err(|_| initial(plain(native::TLV_ERR_OUT_OF_MEMORY).unwrap_err()))?;
        targets.resize(capacity, ptr::null_mut());
        let (kind, tag, value) = match edit {
            QueryEdit::Remove => (0, &[][..], &[][..]),
            QueryEdit::Replace(value) => (1, &[][..], value),
            QueryEdit::InsertAfter { tag, value } => (2, tag, value),
        };
        let mut applied = 0;
        let mut diagnostic = unsafe { zeroed() };
        // SAFETY: &mut Document excludes external node borrows. The private
        // execution has yielded no Nodes; C collects/checks targets before writes.
        let rc = unsafe {
            native::tlv_document_query_edit(
                raw,
                execution.raw,
                kind,
                native::tlv_tag_t {
                    data: tag.as_ptr(),
                    size: tag.len(),
                },
                value.as_ptr(),
                value.len(),
                targets.as_mut_ptr(),
                capacity,
                &mut applied,
                &mut diagnostic,
            )
        };
        check(rc, &diagnostic).map_err(|failure| QueryEditError { failure, applied })?;
        Ok(applied)
    }
    /// Compile bounded UTF-8 text using caller-selected C options. Static
    /// `names` compile directly through `tlv_query_compile()`; a `resolver`
    /// uses checked preparation and commit, so a resolver change between
    /// sizing and writing is detected. Both produce the same image.
    pub fn compile(text: &str, options: &ProgramOptions) -> ProgramResult<Self> {
        Self::build(text.as_bytes(), options, Source::Text)
    }
    /// Validate and own a same-release image copy; release-limited native layout, not serialization.
    pub fn load(image: &[u8], options: &ProgramOptions) -> ProgramResult<Self> {
        Self::build(image, options, Source::Image)
    }
    /// Own a copy of a same-release image and validate only its native plan
    /// through `tlv_query_plan_open()`, without the Query frontend: no parsing,
    /// name resolution or planning. `options` supply the Format, semantic Tags
    /// and providers that executions use; their compatibility is checked when an
    /// execution is created. `info()` then reports the plan's requirements.
    pub fn open_plan(image: &[u8], options: &ProgramOptions) -> ProgramResult<Self> {
        Self::build(image, options, Source::Plan)
    }
    fn build(data: &[u8], options: &ProgramOptions, source: Source) -> ProgramResult<Self> {
        // SAFETY: builtin callbacks/descriptors are immutable static C objects.
        let mut count = 0;
        let mut hooks = unsafe {
            slice::from_raw_parts(native::tlv_query_builtin_hooks(&mut count), count).to_vec()
        };
        let format_owner = match (&options.fixed_format, &options.composed_format) {
            (None, None) => FormatOwner::Builtin(options.format),
            (Some(format), None) => FormatOwner::Fixed(format.clone()),
            (None, Some(format)) => FormatOwner::Composed(format.clone()),
            (Some(_), Some(_)) => return Err(plain(native::TLV_ERR_INVALID_ARG).unwrap_err()),
        };
        let mut tags = if matches!(
            format_owner,
            FormatOwner::Builtin(Format::Ber | Format::Cer | Format::Der)
        ) {
            hooks.push(unsafe { native::tlv_asn1_query_date });
            ptr::addr_of!(native::tlv_asn1_query_tags)
        } else {
            ptr::null()
        };
        let tag_owner = options
            .tags
            .clone()
            .map(providers::TagOwner::new)
            .transpose()?;
        if let Some(owner) = &tag_owner {
            tags = &owner.native;
        }
        let providers = options.providers.clone().into_boxed_slice();
        for (index, provider) in providers.iter().enumerate() {
            if provider.id == 0
                || providers[..index]
                    .iter()
                    .any(|other| other.conversion == provider.conversion || other.id == provider.id)
            {
                return Err(plain(native::TLV_ERR_INVALID_ARG).unwrap_err());
            }
            let (scratch_alignment, context, decode): (_, _, native::tlv_query_decode_t) =
                match &provider.decode {
                    Decoder::Callback(_) => (
                        1,
                        (provider as *const QueryProvider).cast(),
                        Some(decode_provider),
                    ),
                    Decoder::Codec(owner) => (
                        std::mem::align_of::<u64>(),
                        ptr::addr_of!(owner.adapter).cast(),
                        Some(native::tlv_query_codec_decode),
                    ),
                };
            let hook = native::tlv_query_hook_t {
                id: provider.id,
                function: provider.conversion as i32,
                scratch_size: provider.max_result_bytes,
                scratch_alignment,
                context,
                decode,
            };
            if let Some(slot) = hooks
                .iter_mut()
                .find(|hook| hook.function == provider.conversion as i32)
            {
                *slot = hook;
            } else {
                hooks.push(hook);
            }
        }
        let hooks = hooks.into_boxed_slice();
        let environment = native::tlv_query_environment_t {
            format: format_owner.raw(),
            tags,
            hooks: hooks.as_ptr(),
            hook_count: hooks.len(),
        };
        let names: Vec<_> = options
            .variables
            .keys()
            .map(|s| CString::new(s.as_str()))
            .collect::<std::result::Result<_, _>>()
            .map_err(|_| plain(native::TLV_ERR_INVALID_ARG).unwrap_err())?;
        let variables: Vec<_> = names
            .iter()
            .zip(options.variables.values())
            .map(|(n, t)| native::tlv_query_variable_t {
                name: n.as_ptr(),
                type_: *t as i32,
            })
            .collect();
        // SAFETY: zero initializes optional pointers; all borrowed options survive both passes.
        let mut config: native::tlv_query_compile_options_t = unsafe { zeroed() };
        unsafe {
            native::tlv_query_compile_options_init(&mut config);
        }
        config.max_text = options.max_text;
        config.max_tokens = options.max_tokens;
        config.max_nesting = options.max_nesting;
        config.max_states = options.max_states;
        config.max_pattern = options.max_pattern;
        config.max_resolved_tag = options.max_resolved_tag;
        config.variables = variables.as_ptr();
        config.variable_count = variables.len();
        config.environment = &environment;
        config.resolve = Some(resolve);
        config.resolve_context = ptr::addr_of!(options.names).cast();
        let resolver_call = options
            .resolver
            .as_ref()
            .map(|resolver| providers::ResolverCall {
                resolver,
                tag: std::cell::RefCell::new(crate::Tag::default()),
            });
        if let Some(resolver) = &resolver_call {
            config.resolve = Some(providers::resolve_callback);
            config.resolve_context = ptr::addr_of!(*resolver).cast();
        }
        config.optimize = options.optimize as i32;
        let (mut bytes, mut alignment) = (0, 0);
        let mut diagnostic = unsafe { zeroed() };
        let mut info: native::tlv_query_program_info_t = unsafe { zeroed() };
        info.struct_size = size_of::<native::tlv_query_program_info_t>();
        let copy = |memory: &mut Memory| unsafe {
            ptr::copy_nonoverlapping(data.as_ptr(), memory.data_mut().cast::<u8>(), data.len());
        };
        // SAFETY (all native calls below): text, image and options are borrowed
        // for each synchronous call; scratch and program storage are exclusive,
        // 16-byte aligned and disjoint from them.
        let memory = match source {
            Source::Plan => {
                let mut memory = Memory::new(data.len())?;
                copy(&mut memory);
                let mut program = ptr::null();
                check(
                    unsafe {
                        native::tlv_query_plan_open(
                            memory.data(),
                            memory.bytes,
                            &mut program,
                            &mut diagnostic,
                        )
                    },
                    &diagnostic,
                )?;
                plain(unsafe { native::tlv_query_plan_info(program, &mut info) })?;
                memory
            }
            Source::Text if options.resolver.is_none() => {
                // Static names resolve identically in the sizing and writing
                // passes, so the checked prepare/commit recompilation is unnecessary.
                let rc = unsafe {
                    native::tlv_query_compile_scratch(
                        data.as_ptr().cast(),
                        data.len(),
                        &config,
                        &mut bytes,
                        &mut alignment,
                        &mut diagnostic,
                    )
                };
                check(rc, &diagnostic)?;
                let mut workspace = Memory::new(bytes)?;
                let scratch = workspace.data_mut();
                let compile =
                    |storage: *mut c_void,
                     capacity: usize,
                     info: &mut native::tlv_query_program_info_t,
                     diagnostic: &mut native::tlv_query_diagnostic_t| unsafe {
                        native::tlv_query_compile(
                            data.as_ptr().cast(),
                            data.len(),
                            &config,
                            scratch,
                            bytes,
                            storage,
                            capacity,
                            info,
                            diagnostic,
                        )
                    };
                // A NULL, zero-capacity pass sizes the program.
                let rc = compile(ptr::null_mut(), 0, &mut info, &mut diagnostic);
                check(rc, &diagnostic)?;
                let mut memory = Memory::new(info.program_size)?;
                let rc = compile(memory.data_mut(), memory.bytes, &mut info, &mut diagnostic);
                check(rc, &diagnostic)?;
                memory
            }
            Source::Image | Source::Text => {
                let image = matches!(source, Source::Image);
                let rc = unsafe {
                    if image {
                        native::tlv_query_program_load_scratch(
                            data.as_ptr().cast(),
                            data.len(),
                            &config,
                            &mut bytes,
                            &mut alignment,
                            &mut diagnostic,
                        )
                    } else {
                        native::tlv_query_compile_prepare_size(
                            data.as_ptr().cast(),
                            data.len(),
                            &config,
                            &mut bytes,
                            &mut alignment,
                            &mut diagnostic,
                        )
                    }
                };
                check(rc, &diagnostic)?;
                let mut scratch = Memory::new(bytes)?;
                let mut prepared = ptr::null();
                if !image {
                    let rc = unsafe {
                        native::tlv_query_compile_prepare(
                            data.as_ptr().cast(),
                            data.len(),
                            &config,
                            scratch.data_mut(),
                            bytes,
                            &mut prepared,
                            &mut info,
                            &mut diagnostic,
                        )
                    };
                    check(rc, &diagnostic)?;
                }
                let mut memory = Memory::new(if image { data.len() } else { info.program_size })?;
                let rc = if image {
                    copy(&mut memory);
                    let mut loaded = ptr::null();
                    unsafe {
                        native::tlv_query_program_load(
                            memory.data(),
                            memory.bytes,
                            &config,
                            scratch.data_mut(),
                            bytes,
                            &mut loaded,
                            &mut info,
                            &mut diagnostic,
                        )
                    }
                } else {
                    let mut validation_bytes = 0;
                    let rc = unsafe {
                        native::tlv_query_program_load_scratch(
                            prepared.cast(),
                            info.program_size,
                            &config,
                            &mut validation_bytes,
                            &mut alignment,
                            &mut diagnostic,
                        )
                    };
                    check(rc, &diagnostic)?;
                    let mut validation = Memory::new(validation_bytes)?;
                    unsafe {
                        native::tlv_query_compile_commit(
                            prepared.cast(),
                            info.program_size,
                            &config,
                            validation.data_mut(),
                            validation_bytes,
                            memory.data_mut(),
                            memory.bytes,
                            &mut info,
                            &mut diagnostic,
                        )
                    }
                };
                check(rc, &diagnostic)?;
                memory
            }
        };
        Ok(Self {
            storage: Arc::new(ProgramStorage {
                memory,
                info,
                environment,
                _hooks: hooks,
                _providers: providers,
                _tags: tag_owner,
                _format: format_owner,
            }),
        })
    }
    fn raw(&self) -> *const native::tlv_query_program_t {
        self.storage.memory.data().cast()
    }
    /// Read native requirements or execution counters and validation coverage.
    pub fn info(&self) -> &native::tlv_query_program_info_t {
        &self.storage.info
    }
    /// Read requirements from the immutable plan itself through
    /// `tlv_query_plan_info()`. Compiler scratch and optimization-history fields
    /// are zero; execution sizing still takes depth and node limits.
    pub fn plan_info(&self) -> ProgramResult<native::tlv_query_program_info_t> {
        // SAFETY: all-zero info is a valid output extent once struct_size is set.
        let mut info: native::tlv_query_program_info_t = unsafe { zeroed() };
        info.struct_size = size_of::<native::tlv_query_program_info_t>();
        // SAFETY: live validated program and writable output.
        plain(unsafe { native::tlv_query_plan_info(self.raw(), &mut info) })?;
        Ok(info)
    }
    /// Owned unique referenced variable names and types in first-reference order.
    pub fn variables(&self) -> ProgramResult<Vec<(String, QueryType)>> {
        let mut result = Vec::new();
        let count = unsafe { native::tlv_query_program_variable_count(self.raw()) };
        for index in 0..count {
            let mut variable: native::tlv_query_variable_info_t = unsafe { zeroed() };
            plain(unsafe { native::tlv_query_program_variable(self.raw(), index, &mut variable) })?;
            let bytes = unsafe { slice::from_raw_parts(variable.name.cast(), variable.name_size) };
            let name = std::str::from_utf8(bytes)
                .map_err(|_| plain(native::TLV_ERR_INVALID_VALUE).unwrap_err())?
                .to_owned();
            let kind = match variable.type_ {
                2 => QueryType::Integer,
                3 => QueryType::Bytes,
                4 => QueryType::String,
                _ => return Err(plain(native::TLV_ERR_INVALID_ARG).unwrap_err()),
            };
            result.push((name, kind));
        }
        Ok(result)
    }
    /// Borrow immutable native-layout image bytes; compatibility is release-limited.
    pub fn image(&self) -> &[u8] {
        // SAFETY: immutable aligned owned image remains alive through this borrow.
        unsafe {
            slice::from_raw_parts(self.storage.memory.data().cast(), self.storage.memory.bytes)
        }
    }
    /// Return canonical language spelling from C.
    pub fn format(&self) -> ProgramResult<String> {
        self.render(false)
    }
    /// Return implementation-specific C plan details.
    pub fn explain(&self) -> ProgramResult<String> {
        self.render(true)
    }
    fn render(&self, explain: bool) -> ProgramResult<String> {
        let function = if explain {
            native::tlv_query_program_explain
        } else {
            native::tlv_query_program_format
        };
        let mut size = 0;
        plain(unsafe { function(self.raw(), ptr::null_mut(), 0, &mut size) })?;
        let mut output = vec![0u8; size];
        plain(unsafe { function(self.raw(), output.as_mut_ptr().cast(), size, &mut size) })?;
        output.truncate(size - 1);
        String::from_utf8(output).map_err(|_| plain(native::TLV_ERR_INVALID_VALUE).unwrap_err())
    }
    /// Discover exact caller workspace bytes and alignment.
    pub fn workspace_size(
        &self,
        depth: usize,
        nodes: usize,
        retained: bool,
    ) -> ProgramResult<(usize, usize)> {
        let (mut bytes, mut alignment) = (0, 0);
        let rc = unsafe {
            if retained {
                native::tlv_query_eval_size(self.raw(), depth, nodes, &mut bytes, &mut alignment)
            } else {
                native::tlv_query_exec_size(self.raw(), depth, &mut bytes, &mut alignment)
            }
        };
        plain(rc)?;
        Ok((bytes, alignment))
    }
    /// Create independent owning execution with explicit resource bounds.
    pub fn execution<'a>(
        &self,
        depth: usize,
        nodes: usize,
        work: usize,
        retained: bool,
    ) -> ProgramResult<QueryExecution<'a>> {
        let (bytes, _) = self.workspace_size(depth, nodes, retained)?;
        let mut memory = Memory::new(bytes)?;
        let raw = initialize(self, memory.data_mut(), bytes, depth, nodes, work, retained)?;
        Ok(QueryExecution {
            program: self.clone(),
            raw,
            _memory: Some(memory),
            _external: None,
            #[cfg(feature = "document")]
            document: None,
            #[cfg(feature = "document")]
            values: None,
            #[cfg(feature = "document")]
            depth,
            retained,
            input: PhantomData,
        })
    }
    /// Borrow exclusive aligned caller workspace. Returned execution cannot outlive it.
    pub fn execution_external<'a>(
        &self,
        storage: &'a mut [u8],
        depth: usize,
        nodes: usize,
        work: usize,
        retained: bool,
    ) -> ProgramResult<QueryExecution<'a>> {
        let raw = initialize(
            self,
            storage.as_mut_ptr().cast(),
            storage.len(),
            depth,
            nodes,
            work,
            retained,
        )?;
        Ok(QueryExecution {
            program: self.clone(),
            raw,
            _memory: None,
            _external: Some(storage),
            #[cfg(feature = "document")]
            document: None,
            #[cfg(feature = "document")]
            values: None,
            #[cfg(feature = "document")]
            depth,
            retained,
            input: PhantomData,
        })
    }
}
fn initialize(
    program: &QueryProgram,
    storage: *mut c_void,
    capacity: usize,
    depth: usize,
    nodes: usize,
    work: usize,
    retained: bool,
) -> ProgramResult<*mut native::tlv_query_exec_t> {
    let mut raw = ptr::null_mut();
    let rc = unsafe {
        if retained {
            native::tlv_query_eval_init(
                program.raw(),
                &program.storage.environment,
                storage,
                capacity,
                depth,
                nodes,
                work,
                &mut raw,
            )
        } else {
            native::tlv_query_exec_init(
                program.raw(),
                storage,
                capacity,
                depth,
                nodes,
                work,
                &mut raw,
            )
        }
    };
    plain(rc)?;
    Ok(raw)
}
/// Borrowed typed binding; spans must survive the execution's complete input lifetime.
pub enum QueryBinding<'a> {
    /// Signed 64-bit integer.
    Integer(i64),
    /// Byte span.
    Bytes(&'a [u8]),
    /// Validated UTF-8 string.
    String(&'a str),
}
/// Finalized scalar spans borrow execution/program/input; copy explicitly for ownership.
#[derive(Debug, PartialEq, Eq)]
pub enum QueryValue<'a> {
    /// Boolean scalar.
    Boolean(bool),
    /// Signed 64-bit integer.
    Integer(i64),
    /// Byte span.
    Bytes(&'a [u8]),
    /// Validated UTF-8 string.
    String(&'a str),
}
/// Selected borrowed node; payload cannot outlive this execution borrow.
#[derive(Debug)]
pub struct QueryMatch<'a> {
    /// Selected semantic Element with a borrowed Value.
    pub element: Element<'a>,
    /// Zero-based node depth.
    pub depth: usize,
    /// Absolute input offset.
    pub offset: usize,
    /// Whether this node is a BEGIN event.
    pub constructed: bool,
}
/// Borrowed canonical event without wire Source metadata. Producers retain all
/// Tag and complete Value bytes until the execution is reset or dropped.
#[derive(Clone, Copy, Debug)]
pub enum QueryEvent<'a> {
    /// Open a constructed node; Value contains its complete logical content.
    Begin {
        /// Raw Tag bytes.
        tag: &'a [u8],
        /// Complete Value bytes.
        value: &'a [u8],
        /// Zero-based node depth.
        depth: usize,
        /// Producer-supplied event offset, without wire Source semantics.
        offset: usize,
    },
    /// One primitive node.
    Element {
        /// Raw Tag bytes.
        tag: &'a [u8],
        /// Complete Value bytes.
        value: &'a [u8],
        /// Zero-based node depth.
        depth: usize,
        /// Producer-supplied event offset, without wire Source semantics.
        offset: usize,
    },
    /// Close the innermost constructed node.
    End {
        /// Depth of the node being closed.
        depth: usize,
        /// Producer-supplied end offset.
        offset: usize,
        /// Descendants were omitted rather than validated.
        skipped: bool,
    },
}
unsafe fn project<'a>(event: &native::tlv_tree_event_t) -> ProgramResult<QueryMatch<'a>> {
    Ok(QueryMatch {
        element: unsafe { Element::from_raw(&event.element) }.map_err(|e| ProgramError {
            location: crate::Location::default(),
            metadata: None,
            error: e,
            kind: crate::QueryErrorKind::None,
            expression: crate::Location::default(),
            limit: None,
            codec_detail: None,
            reader: None,
        })?,
        depth: event.depth,
        offset: event.offset,
        constructed: event.kind == native::TLV_TREE_BEGIN,
    })
}
/// Exclusive mutable continuation, borrowing all input windows and variable spans for `'a`.
pub struct QueryExecution<'a> {
    program: QueryProgram,
    raw: *mut native::tlv_query_exec_t,
    _memory: Option<Memory>,
    _external: Option<&'a mut [u8]>,
    #[cfg(feature = "document")]
    document: Option<&'a crate::Document<'a>>,
    #[cfg(feature = "document")]
    values: Option<Memory>,
    #[cfg(feature = "document")]
    depth: usize,
    retained: bool,
    input: PhantomData<&'a [u8]>,
}
impl<'a> QueryExecution<'a> {
    /// Feed a complete canonical event whose payload and owned Tag remain live for `'a`.
    /// Immediate selection borrows this execution; retained results wait for finish().
    pub fn feed<'s>(
        &'s mut self,
        event: &'a crate::TreeEvent<'a>,
    ) -> ProgramResult<Option<QueryMatch<'s>>> {
        self.tree_backend()?;
        let mut raw: native::tlv_tree_event_t = unsafe { zeroed() };
        match event {
            crate::TreeEvent::Begin(item) | crate::TreeEvent::Element(item) => {
                raw.kind = if matches!(event, crate::TreeEvent::Begin(_)) {
                    native::TLV_TREE_BEGIN
                } else {
                    native::TLV_TREE_ELEMENT
                };
                raw.element = native::tlv_element_t {
                    tag: item.decoded.element.tag().raw(),
                    value: native::tlv_value_t {
                        data: item.decoded.element.value().as_ptr(),
                        size: item.decoded.element.value().len() as u64,
                    },
                };
                raw.source = item.decoded.source;
                raw.depth = item.depth;
                raw.offset = item.offset;
            }
            crate::TreeEvent::End {
                depth,
                offset,
                skipped,
            } => {
                raw.kind = native::TLV_TREE_END;
                raw.depth = *depth;
                raw.offset = *offset;
                raw.skipped = *skipped as i32;
            }
        }
        self.feed_raw(raw)
    }
    /// Feed a source-less canonical event, retaining its borrowed payload for
    /// the execution lifetime. The C engine validates event balance and bounds.
    pub fn feed_event(&mut self, event: QueryEvent<'a>) -> ProgramResult<Option<QueryMatch<'_>>> {
        self.tree_backend()?;
        let mut raw: native::tlv_tree_event_t = unsafe { zeroed() };
        match event {
            QueryEvent::Begin {
                tag,
                value,
                depth,
                offset,
            }
            | QueryEvent::Element {
                tag,
                value,
                depth,
                offset,
            } => {
                raw.kind = if matches!(event, QueryEvent::Begin { .. }) {
                    native::TLV_TREE_BEGIN
                } else {
                    native::TLV_TREE_ELEMENT
                };
                raw.element = native::tlv_element_t {
                    tag: native::tlv_tag_t {
                        data: tag.as_ptr(),
                        size: tag.len(),
                    },
                    value: native::tlv_value_t {
                        data: value.as_ptr(),
                        size: value.len() as u64,
                    },
                };
                raw.depth = depth;
                raw.offset = offset;
            }
            QueryEvent::End {
                depth,
                offset,
                skipped,
            } => {
                raw.kind = native::TLV_TREE_END;
                raw.depth = depth;
                raw.offset = offset;
                raw.skipped = skipped as i32;
            }
        }
        self.feed_raw(raw)
    }
    fn feed_raw(
        &mut self,
        mut raw: native::tlv_tree_event_t,
    ) -> ProgramResult<Option<QueryMatch<'_>>> {
        let mut matched = 0;
        let mut diagnostic = unsafe { zeroed() };
        let rc =
            unsafe { native::tlv_query_exec_feed(self.raw, &raw, &mut matched, &mut diagnostic) };
        check(rc, &diagnostic)?;
        if matched == 0 {
            return Ok(None);
        }
        if !self.retained && self.program.info().level == 1 {
            plain(unsafe { native::tlv_query_exec_selected(self.raw, &mut raw) })?;
        }
        unsafe { project(&raw) }.map(Some)
    }
    /// Require balanced final events and finalize retained evaluation.
    pub fn finish(&mut self) -> ProgramResult<()> {
        self.tree_backend()?;
        let mut diagnostic = unsafe { zeroed() };
        let rc = unsafe { native::tlv_query_exec_finish(self.raw, &mut diagnostic) };
        check(rc, &diagnostic)
    }
    /// Pull a finalized retained node after raw event feeding, without a Reader.
    /// Returns None at exhaustion; before finish, reports InvalidState with Query STATE detail.
    pub fn next_result(&mut self) -> ProgramResult<Option<QueryMatch<'_>>> {
        self.tree_backend()?;
        let mut event = unsafe { zeroed() };
        // SAFETY: exclusive execution borrow protects its cursor, and feed/Reader
        // lifetimes retain every input span projected into the returned match.
        let code = unsafe { native::tlv_query_result_next(self.raw, &mut event) };
        if code == native::TLV_END {
            return Ok(None);
        }
        plain(code)?;
        unsafe { project(&event) }.map(Some)
    }
    /// Pull a finalized retained node and its original preorder identity. The
    /// ordinal can select that node as context in another execution over the same
    /// event sequence. Shares the result cursor with all other pull operations.
    pub fn next_result_with_ordinal(&mut self) -> ProgramResult<Option<(QueryMatch<'_>, usize)>> {
        self.tree_backend()?;
        let mut event = unsafe { zeroed() };
        let mut ordinal = 0;
        // SAFETY: exclusive execution borrow protects its cursor, and feed/Reader
        // lifetimes retain every input span projected into the returned match.
        let code =
            unsafe { native::tlv_query_result_next_ordinal(self.raw, &mut event, &mut ordinal) };
        if code == native::TLV_END {
            return Ok(None);
        }
        plain(code)?;
        unsafe { project(&event) }.map(|matched| Some((matched, ordinal)))
    }
    /// Reset in place through `tlv_query_exec_reset()`, keeping the program,
    /// providers, workspace and budgets. Clears bindings, context, pruning,
    /// counters, retained input and results, and releases a Document snapshot;
    /// a finished or failed execution becomes fresh again.
    pub fn reset(&mut self) -> ProgramResult<()> {
        // SAFETY: exclusive live execution; no callback can run while it is borrowed.
        plain(unsafe { native::tlv_query_exec_reset(self.raw) })?;
        #[cfg(feature = "document")]
        {
            self.document = None;
            self.values = None;
        }
        Ok(())
    }
    /// Bind a typed immutable value before consuming input.
    pub fn bind(&mut self, name: &str, value: QueryBinding<'a>) -> ProgramResult<()> {
        let name =
            CString::new(name).map_err(|_| plain(native::TLV_ERR_INVALID_ARG).unwrap_err())?;
        let (kind, number, data) = match value {
            QueryBinding::Integer(n) => (2, n, &[][..]),
            QueryBinding::Bytes(b) => (3, 0, b),
            QueryBinding::String(s) => (4, 0, s.as_bytes()),
        };
        let mut diagnostic = unsafe { zeroed() };
        let rc = unsafe {
            native::tlv_query_exec_bind(
                self.raw,
                name.as_ptr(),
                kind,
                number,
                data.as_ptr(),
                data.len(),
                &mut diagnostic,
            )
        };
        check(rc, &diagnostic)
    }
    /// Select a relative preorder context on fresh execution.
    pub fn context(&mut self, ordinal: usize) -> ProgramResult<()> {
        plain(unsafe { native::tlv_query_exec_context(self.raw, ordinal) })
    }
    /// Explicitly permit proven subtree pruning with partial validation coverage.
    pub fn pruning(&mut self, enabled: bool) -> ProgramResult<()> {
        plain(unsafe { native::tlv_query_exec_pruning(self.raw, enabled as i32) })
    }
    /// Read native requirements or execution counters and validation coverage.
    pub fn info(&self) -> ProgramResult<native::tlv_query_exec_info_t> {
        let mut info: native::tlv_query_exec_info_t = unsafe { zeroed() };
        info.struct_size = size_of::<native::tlv_query_exec_info_t>();
        plain(unsafe { native::tlv_query_exec_info(self.raw, &mut info) })?;
        Ok(info)
    }
    /// Visit borrowed matching nodes; catches panics before returning through C.
    pub fn visit<F>(&mut self, reader: &mut TreeReader<'a>, mut visitor: F) -> ProgramResult<()>
    where
        F: for<'e> FnMut(QueryMatch<'e>) -> Visit,
    {
        self.tree_backend()?;
        struct State<'f, F> {
            callback: &'f mut F,
            panic: Option<Box<dyn std::any::Any + Send>>,
            failure: Option<ProgramError>,
        }
        unsafe extern "C" fn callback<F>(
            event: *const native::tlv_tree_event_t,
            context: *mut c_void,
        ) -> i32
        where
            F: for<'e> FnMut(QueryMatch<'e>) -> Visit,
        {
            // SAFETY: synchronous C call pins state; event spans are live during the callback only.
            let state = unsafe { &mut *context.cast::<State<'_, F>>() };
            let matched = match unsafe { project(&*event) } {
                Ok(value) => value,
                Err(e) => {
                    state.failure = Some(e);
                    return 2;
                }
            };
            match catch_unwind(AssertUnwindSafe(|| (state.callback)(matched))) {
                Ok(action) => action as i32,
                Err(panic) => {
                    state.panic = Some(panic);
                    2
                }
            }
        }
        let mut state = State {
            callback: &mut visitor,
            panic: None,
            failure: None,
        };
        let mut diagnostic = unsafe { zeroed() };
        reader.current = None;
        let rc = unsafe {
            native::tlv_query_program_visit(
                &mut reader.raw,
                self.raw,
                Some(callback::<F>),
                ptr::addr_of_mut!(state).cast(),
                &mut diagnostic,
            )
        };
        if let Some(panic) = state.panic {
            std::panic::resume_unwind(panic);
        }
        if let Some(error) = state.failure {
            return Err(error);
        }
        check(rc, &diagnostic)
    }
    /// Pull one borrowed node through native STOP/resume; None means final exhaustion.
    pub fn next<'s>(
        &'s mut self,
        reader: &mut TreeReader<'a>,
    ) -> ProgramResult<Option<QueryMatch<'s>>> {
        self.tree_backend()?;
        if self.program.info().result_kind != 0 {
            return Err(plain(native::TLV_ERR_INVALID_ARG).unwrap_err());
        }
        unsafe extern "C" fn one(
            event: *const native::tlv_tree_event_t,
            context: *mut c_void,
        ) -> i32 {
            unsafe {
                *context.cast::<Option<native::tlv_tree_event_t>>() = Some(*event);
            }
            1
        }
        let mut selected = None;
        let mut diagnostic = unsafe { zeroed() };
        reader.current = None;
        let rc = unsafe {
            native::tlv_query_program_visit(
                &mut reader.raw,
                self.raw,
                Some(one),
                ptr::addr_of_mut!(selected).cast(),
                &mut diagnostic,
            )
        };
        check(rc, &diagnostic)?;
        selected
            .as_ref()
            .map(|event| unsafe { project(event) })
            .transpose()
    }
    /// Test existence, draining input unless explicit early return is selected.
    pub fn exists(
        &mut self,
        reader: &mut TreeReader<'a>,
        early_return: bool,
    ) -> ProgramResult<bool> {
        self.tree_backend()?;
        reader.current = None;
        let mut diagnostic = unsafe { zeroed() };
        let mut found = 0;
        let rc = unsafe {
            native::tlv_query_program_exists(
                &mut reader.raw,
                self.raw,
                early_return as i32,
                &mut found,
                &mut diagnostic,
            )
        };
        check(rc, &diagnostic)?;
        Ok(found != 0)
    }
    fn tree_backend(&self) -> ProgramResult<()> {
        #[cfg(feature = "document")]
        if self.document.is_some() {
            return plain(native::TLV_ERR_INVALID_ARG);
        }
        Ok(())
    }
    /// Evaluate an immutable Document revision; the shared borrow prevents safe edits/destruction.
    /// Constructed Values use one owned canonical snapshot, with an optional explicit byte bound.
    #[cfg(feature = "document")]
    pub fn evaluate_document(
        &mut self,
        document: &'a crate::Document<'a>,
        context: Option<&crate::Node<'_, 'a>>,
        value_capacity: Option<usize>,
    ) -> ProgramResult<()> {
        if self.document.is_some() {
            return plain(native::TLV_ERR_INVALID_STATE);
        }
        if !self.retained {
            return plain(native::TLV_ERR_INVALID_ARG);
        }
        let mut frames = Vec::<native::tlv_tree_writer_frame_t>::new();
        let mut staged = Memory::new(0)?;
        let mut scratch = Memory::new(0)?;
        let mut values = Memory::new(0)?;
        let mut workspace: native::tlv_tree_writer_workspace_t = unsafe { zeroed() };
        let mut capacity = 0;
        let mut diagnostic = unsafe { zeroed() };
        if self.program.info().constructed_values_required != 0 {
            let frame_count = self
                .depth
                .checked_add(1)
                .ok_or_else(|| plain(native::TLV_ERR_OVERFLOW).unwrap_err())?;
            frames
                .try_reserve_exact(frame_count)
                .map_err(|_| plain(native::TLV_ERR_OUT_OF_MEMORY).unwrap_err())?;
            frames.resize_with(frame_count, || unsafe { zeroed() });
            workspace.frames = frames.as_mut_ptr();
            workspace.frame_capacity = frames.len();
            if let Some(value) = value_capacity {
                capacity = value;
                staged = Memory::new(capacity)?;
                scratch = Memory::new(capacity)?;
                values = Memory::new(capacity)?;
                workspace.data = staged.data_mut().cast();
                workspace.data_capacity = capacity;
                workspace.scratch = scratch.data_mut().cast();
                workspace.scratch_capacity = capacity;
            } else {
                // Grow staging to the Writer's reported requirements until the
                // complete snapshot fits; that output then is the snapshot.
                loop {
                    workspace.data = staged.data_mut().cast();
                    workspace.data_capacity = staged.bytes;
                    workspace.scratch = scratch.data_mut().cast();
                    workspace.scratch_capacity = scratch.bytes;
                    // SAFETY: live immutable Document and exclusive staging storage.
                    let rc = unsafe {
                        native::tlv_document_query_value_size(
                            document.raw,
                            &mut workspace,
                            &mut capacity,
                            &mut diagnostic,
                        )
                    };
                    let (data, extra) = (workspace.required_data, workspace.required_scratch);
                    if rc != native::TLV_ERR_BUFFER_TOO_SHORT
                        || (data <= staged.bytes && extra <= scratch.bytes)
                    {
                        check(rc, &diagnostic)?;
                        break;
                    }
                    if data > staged.bytes {
                        staged = Memory::new(data)?;
                    }
                    if extra > scratch.bytes {
                        scratch = Memory::new(extra)?;
                    }
                }
                // Moving the owner keeps the buffer, so staging still names it.
                values = std::mem::replace(&mut staged, Memory::new(0)?);
            }
        }
        self.document = Some(document);
        let rc = unsafe {
            native::tlv_document_query_evaluate(
                document.raw,
                self.raw,
                context.map_or(ptr::null(), |node| node.raw),
                values.data_mut(),
                capacity,
                if self.program.info().constructed_values_required != 0 {
                    &mut workspace
                } else {
                    ptr::null_mut()
                },
                &mut diagnostic,
            )
        };
        // Preserve snapshot storage even if evaluation failed after borrowing it.
        self.values = Some(values);
        check(rc, &diagnostic)
    }
    /// Pull a checked Document handle in native preorder; None means final exhaustion.
    #[cfg(feature = "document")]
    pub fn next_document(&mut self) -> ProgramResult<Option<crate::Node<'a, 'a>>> {
        let document = self
            .document
            .ok_or_else(|| plain(native::TLV_ERR_INVALID_ARG).unwrap_err())?;
        let mut node = ptr::null_mut();
        let mut diagnostic = unsafe { zeroed() };
        let rc = unsafe { native::tlv_document_query_next(self.raw, &mut node, &mut diagnostic) };
        if rc == native::TLV_END {
            return Ok(None);
        }
        check(rc, &diagnostic)?;
        Ok(document.node(node))
    }
    /// Visit the remaining finalized Document results through
    /// `tlv_document_query_program_visit()`. It shares the result cursor with
    /// [`QueryExecution::next_document`]; `Visit::Stop` ends the call and a later
    /// pull or visit resumes after the delivered node. A visitor panic resumes
    /// after C returns.
    #[cfg(feature = "document")]
    pub fn visit_document<F>(&mut self, mut visitor: F) -> ProgramResult<()>
    where
        F: FnMut(crate::Node<'a, 'a>) -> Visit,
    {
        let document = self
            .document
            .ok_or_else(|| plain(native::TLV_ERR_INVALID_ARG).unwrap_err())?;
        let raw = self.raw;
        let mut diagnostic = unsafe { zeroed() };
        // SAFETY: exclusive execution; the evaluated Document stays shared-borrowed
        // for 'a, so callbacks cannot edit it, and C delivers only its live nodes.
        let rc = unsafe {
            crate::document::visit_nodes(
                |node| visitor(document.node(node).expect("C delivers non-null nodes")),
                |function, context| {
                    native::tlv_document_query_program_visit(
                        raw,
                        function,
                        context,
                        &mut diagnostic,
                    )
                },
            )
        };
        check(rc, &diagnostic)
    }
    /// Read a finalized scalar borrowing execution/input/program storage.
    pub fn result(&self) -> ProgramResult<QueryValue<'_>> {
        let mut result: native::tlv_query_result_t = unsafe { zeroed() };
        plain(unsafe { native::tlv_query_exec_result(self.raw, &mut result) })?;
        // SAFETY: successful finalized spans borrow this execution's live input/program/storage.
        let bytes = if result.size == 0 {
            &[]
        } else {
            unsafe { slice::from_raw_parts(result.data, result.size) }
        };
        match result.kind {
            1 => Ok(QueryValue::Boolean(result.boolean != 0)),
            2 => Ok(QueryValue::Integer(result.integer)),
            3 => Ok(QueryValue::Bytes(bytes)),
            4 => std::str::from_utf8(bytes)
                .map(QueryValue::String)
                .map_err(|_| plain(native::TLV_ERR_INVALID_VALUE).unwrap_err()),
            _ => Err(plain(native::TLV_ERR_INVALID_ARG).unwrap_err()),
        }
    }
}
