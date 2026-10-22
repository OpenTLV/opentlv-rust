// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Owning C Document with lifetime-bound reads and exclusive mutation.
use crate::{Error, FixedFormat, Format, Query, ReaderDiagnostic, Result, Tag, TreeReader};
use opentlv_sys as native;
use std::{marker::PhantomData, mem::MaybeUninit, ptr, slice};

/// Document failure, with a source offset or required output capacity when supplied by C.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentError {
    /// Canonical error.
    pub error: Error,
    /// Absolute source offset for parsing failures.
    pub offset: Option<usize>,
    /// Primary evidence location, copied from C.
    pub location: crate::Location,
    /// Required output capacity when encoding into insufficient storage.
    pub required: Option<usize>,
}
impl From<Error> for DocumentError {
    fn from(error: Error) -> Self {
        Self {
            error,
            offset: None,
            location: crate::Location::default(),
            required: None,
        }
    }
}
impl std::fmt::Display for DocumentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error)?;
        if let Some(offset) = self.offset {
            write!(f, " at byte {offset}")?;
        }
        Ok(())
    }
}
impl std::error::Error for DocumentError {}
type DocResult<T> = std::result::Result<T, DocumentError>;

/// Resumable materialization through C, exclusively borrowing a Tree Reader.
/// While active, input replacement is allowed through this builder; direct pulls
/// and skips are excluded by the mutable borrow. Completed Documents own their
/// content but conservatively retain the reader's Format/input lifetime.
pub struct DocumentBuilder<'r, 'a> {
    raw: *mut native::tlv_document_builder_t,
    reader: &'r mut TreeReader<'a>,
}
impl<'r, 'a> DocumentBuilder<'r, 'a> {
    /// Materialize the item most recently returned by `TreeReader::read` and its subtree.
    /// Call immediately after matching that item. Pulls, input replacement, skips,
    /// visitors and builder creation invalidate the selection, even on failure.
    /// No second pull or decode of the selected root is performed.
    pub fn current_subtree(
        reader: &'r mut TreeReader<'a>,
        max_depth: usize,
        max_elements: usize,
    ) -> DocResult<Self> {
        let root = reader.current.take().ok_or(Error::InvalidState)?;
        Self::create(reader, &root, max_depth, max_elements)
    }

    /// Materialize the whole stream from a fresh Tree Reader.
    pub fn new(
        reader: &'r mut TreeReader<'a>,
        max_depth: usize,
        max_elements: usize,
    ) -> DocResult<Self> {
        Self::create(reader, ptr::null(), max_depth, max_elements)
    }
    /// Pull the next root and materialize just its subtree. C leaves the reader
    /// positioned before the following sibling when consume completes.
    /// A root publication remains consumed if builder allocation fails.
    pub fn next_subtree(
        reader: &'r mut TreeReader<'a>,
        max_depth: usize,
        max_elements: usize,
    ) -> DocResult<Self> {
        reader.current = None;
        let mut root = MaybeUninit::uninit();
        let mut diagnostic = MaybeUninit::uninit();
        // SAFETY: exclusive live reader and writable outputs, input retained by reader.
        let code = unsafe {
            native::tlv_reader_diagnostic_init(diagnostic.as_mut_ptr());
            native::tlv_tree_reader_next_diag(
                &mut reader.raw,
                root.as_mut_ptr(),
                diagnostic.as_mut_ptr(),
            )
        };
        // SAFETY: initialized diagnostic; reader's input is still borrowed.
        let diagnostic = unsafe { ReaderDiagnostic::from_raw(&diagnostic.assume_init()) };
        reader.diagnostic = (code != native::TLV_OK).then_some(diagnostic);
        Error::check(code).map_err(|error| DocumentError {
            error,
            offset: reader.diagnostic.as_ref().and_then(|d| d.offset),
            location: reader
                .diagnostic
                .as_ref()
                .map(|d| d.location)
                .unwrap_or_default(),
            required: None,
        })?;
        // SAFETY: successful pull initialized the complete item. Create immediately
        // before any other cursor operation and copy root content in C.
        let root = unsafe { root.assume_init() };
        Self::create(reader, &root, max_depth, max_elements)
    }
    fn create(
        reader: &'r mut TreeReader<'a>,
        root: *const native::tlv_tree_item_t,
        max_depth: usize,
        max_elements: usize,
    ) -> DocResult<Self> {
        reader.current = None;
        let mut options = MaybeUninit::uninit();
        // SAFETY: initialized reader's immutable Format outlives the resulting document.
        Error::check(unsafe {
            native::tlv_document_options_init(options.as_mut_ptr(), reader.raw.input.format)
        })?;
        // SAFETY: successful initialization.
        let mut options = unsafe { options.assume_init() };
        options.max_depth = max_depth;
        options.max_elements = max_elements;
        let mut raw = ptr::null_mut();
        // SAFETY: exclusive reader borrow keeps its raw cursor stable; root is either
        // NULL or the just-published live native item, copied during this call.
        Error::check(unsafe {
            native::tlv_document_builder_create(&options, &mut reader.raw, root, &mut raw)
        })?;
        Ok(Self { raw, reader })
    }
    /// Consume available input. NeedMoreData retains state; only success transfers
    /// a Document. Terminal errors discard unfinished nodes in C.
    pub fn consume(&mut self) -> DocResult<Document<'a>> {
        let mut raw = ptr::null_mut();
        let mut diagnostic = MaybeUninit::uninit();
        // SAFETY: live builder and exclusively borrowed stable reader, valid outputs.
        let code = unsafe {
            native::tlv_reader_diagnostic_init(diagnostic.as_mut_ptr());
            native::tlv_document_builder_consume(self.raw, &mut raw, diagnostic.as_mut_ptr())
        };
        // SAFETY: input remains live through copying all diagnostic fields.
        self.reader.diagnostic = (code != native::TLV_OK)
            .then(|| unsafe { ReaderDiagnostic::from_raw(&diagnostic.assume_init()) });
        Error::check(code).map_err(|error| DocumentError {
            error,
            offset: self
                .reader
                .diagnostic
                .as_ref()
                .and_then(|d| d.location.offset()),
            location: self
                .reader
                .diagnostic
                .as_ref()
                .map(|d| d.location)
                .unwrap_or_default(),
            required: None,
        })?;
        Ok(Document {
            raw,
            format: self.reader.raw.input.format,
            lifetime: PhantomData,
        })
    }
    /// Replace input using the same preserved-suffix checks as TreeReader.
    pub fn set_input(&mut self, input: &'a [u8], discard: usize, final_input: bool) -> Result<()> {
        self.reader.set_input(input, discard, final_input)
    }
    /// Discardable prefix of the current input window.
    pub fn consumed(&self) -> usize {
        self.reader.consumed()
    }
    /// Absolute source frontier.
    pub fn offset(&self) -> usize {
        self.reader.offset()
    }
    /// Owned Reader diagnostic for the latest failure, when available.
    pub fn diagnostic(&self) -> Option<&ReaderDiagnostic> {
        self.reader.diagnostic.as_ref()
    }
}
impl Drop for DocumentBuilder<'_, '_> {
    fn drop(&mut self) {
        // SAFETY: sole owning builder handle; reader remains borrowed until after drop.
        unsafe { native::tlv_document_builder_free(self.raw) };
    }
}

/// C-owned mutable tree. Input is copied by C; a Fixed Format is borrowed.
///
/// Immutable nodes and values borrow the document. Mutations require exclusive
/// access, so retained reads cannot be invalidated by erasing or replacing nodes.
///
/// ```compile_fail
/// use opentlv::{Document, Format};
/// let mut document = Document::parse(&[1, 1, 42], Format::Ber, 64, 100).unwrap();
/// let node = document.first().unwrap();
/// document.first_mut().unwrap().erase();
/// println!("{:?}", node.value());
/// ```
pub struct Document<'f> {
    pub(crate) raw: *mut native::tlv_document_t,
    format: *const native::tlv_format_t,
    lifetime: PhantomData<&'f native::tlv_format_t>,
}

impl Document<'static> {
    /// Create an empty document with explicit depth and node limits.
    pub fn new(format: Format, max_depth: usize, max_elements: usize) -> DocResult<Self> {
        // SAFETY: builtin descriptor is static.
        unsafe { Self::init(None, format.raw(), max_depth, max_elements, false) }
    }
    /// Copy and parse input through the C Document engine.
    pub fn parse(
        data: &[u8],
        format: Format,
        max_depth: usize,
        max_elements: usize,
    ) -> DocResult<Self> {
        // SAFETY: builtin descriptor is static; C copies all input.
        unsafe { Self::init(Some(data), format.raw(), max_depth, max_elements, false) }
    }

    /// Parse with original offsets/header lengths available to Document Query.
    /// No input bytes are borrowed. Edits invalidate affected nodes and ancestors;
    /// unaffected nodes keep their original input coordinates.
    pub fn parse_with_source_locations(
        data: &[u8],
        format: Format,
        max_depth: usize,
        max_elements: usize,
    ) -> DocResult<Self> {
        // SAFETY: builtin descriptor is static; C copies all input and metadata.
        unsafe { Self::init(Some(data), format.raw(), max_depth, max_elements, true) }
    }
}

impl<'f> Document<'f> {
    /// Create an empty document borrowing a Fixed Format for its entire lifetime.
    pub fn with_fixed_format(
        format: &'f FixedFormat<'_>,
        max_depth: usize,
        max_elements: usize,
    ) -> DocResult<Self> {
        // SAFETY: signature retains descriptor and context for 'f.
        unsafe { Self::init(None, format.raw(), max_depth, max_elements, false) }
    }
    /// Parse copied input while borrowing the Fixed descriptor and context.
    pub fn parse_fixed(
        data: &[u8],
        format: &'f FixedFormat<'_>,
        max_depth: usize,
        max_elements: usize,
    ) -> DocResult<Self> {
        // SAFETY: signature retains descriptor and context for 'f.
        unsafe { Self::init(Some(data), format.raw(), max_depth, max_elements, false) }
    }

    /// Parse with original source coordinates, borrowing only the Fixed Format.
    /// See `Document::parse_with_source_locations` for edit invalidation.
    pub fn parse_fixed_with_source_locations(
        data: &[u8],
        format: &'f FixedFormat<'_>,
        max_depth: usize,
        max_elements: usize,
    ) -> DocResult<Self> {
        // SAFETY: signature retains the descriptor/context; C copies input metadata.
        unsafe { Self::init(Some(data), format.raw(), max_depth, max_elements, true) }
    }
    unsafe fn init(
        data: Option<&[u8]>,
        format: *const native::tlv_format_t,
        max_depth: usize,
        max_elements: usize,
        retain_source_locations: bool,
    ) -> DocResult<Self> {
        let mut options = MaybeUninit::uninit();
        // SAFETY: valid borrowed format and writable options.
        Error::check(unsafe { native::tlv_document_options_init(options.as_mut_ptr(), format) })?;
        // SAFETY: successful C initialization.
        let mut options = unsafe { options.assume_init() };
        options.max_depth = max_depth;
        options.max_elements = max_elements;
        options.retain_source_locations = i32::from(retain_source_locations);
        let mut raw = ptr::null_mut();
        // SAFETY: zero diagnostic has no active borrows.
        let mut diagnostic: native::tlv_reader_diagnostic_t = unsafe { std::mem::zeroed() };
        // SAFETY: C owns copied nodes; all temporary inputs remain live through the call.
        let code = unsafe {
            match data {
                Some(data) => native::tlv_document_parse(
                    data.as_ptr(),
                    data.len(),
                    &options,
                    &mut raw,
                    &mut diagnostic,
                ),
                None => native::tlv_document_create(&options, &mut raw),
            }
        };
        Error::check(code).map_err(|error| DocumentError {
            error,
            offset: crate::Location::from_raw(diagnostic.diagnostic.location).offset(),
            location: crate::Location::from_raw(diagnostic.diagnostic.location),
            required: None,
        })?;
        Ok(Self {
            raw,
            format,
            lifetime: PhantomData,
        })
    }
    /// Number of nodes, including descendants.
    pub fn len(&self) -> usize {
        // SAFETY: owned live document, immutable access.
        unsafe { native::tlv_document_count(self.raw) }
    }
    /// Whether the document has no nodes.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub(crate) fn node(&self, raw: *mut native::tlv_node_t) -> Option<Node<'_, 'f>> {
        (!raw.is_null()).then_some(Node {
            raw,
            document: self,
        })
    }
    fn node_mut(&mut self, raw: *mut native::tlv_node_t) -> Option<NodeMut<'_, 'f>> {
        (!raw.is_null()).then_some(NodeMut {
            raw,
            document: self,
        })
    }
    /// First top-level node, borrowing this document immutably.
    pub fn first(&self) -> Option<Node<'_, 'f>> {
        // SAFETY: immutable access to the owned C document.
        self.node(unsafe { native::tlv_document_first(self.raw) })
    }
    /// First top-level node, borrowing this document exclusively.
    pub fn first_mut(&mut self) -> Option<NodeMut<'_, 'f>> {
        // SAFETY: exclusive access to the owned C document.
        self.node_mut(unsafe { native::tlv_document_first(self.raw) })
    }
    /// First top-level node with the requested Tag; matching is performed by C.
    pub fn find(&self, tag: &Tag) -> Option<Node<'_, 'f>> {
        // SAFETY: document and tag remain live throughout native lookup.
        self.node(unsafe { native::tlv_document_find(self.raw, ptr::null(), tag.raw()) })
    }
    /// First top-level match with exclusive mutation access.
    pub fn find_mut(&mut self, tag: &Tag) -> Option<NodeMut<'_, 'f>> {
        // SAFETY: exclusive document access, valid tag.
        self.node_mut(unsafe { native::tlv_document_find(self.raw, ptr::null(), tag.raw()) })
    }
    /// First path match using the canonical parsed Query engine; no match is `Ok(None)`.
    /// Native validation failures are returned without changing their error category.
    pub fn find_path(&self, query: &Query) -> Result<Option<Node<'_, 'f>>> {
        let mut raw = ptr::null_mut();
        // SAFETY: live query and document; result borrows this document.
        Error::check(unsafe { native::tlv_document_find_path(self.raw, &query.raw, &mut raw) })?;
        Ok(self.node(raw))
    }
    /// First path match with exclusive mutation access; no match is `Ok(None)`.
    /// Native validation failures are returned without changing their error category.
    pub fn find_path_mut(&mut self, query: &Query) -> Result<Option<NodeMut<'_, 'f>>> {
        let mut raw = ptr::null_mut();
        // SAFETY: exclusive document access, valid parsed query.
        Error::check(unsafe { native::tlv_document_find_path(self.raw, &query.raw, &mut raw) })?;
        Ok(self.node_mut(raw))
    }
    /// Append a copied top-level element. Constructed values are parsed by C.
    pub fn append(&mut self, tag: &Tag, value: &[u8]) -> Result<NodeMut<'_, 'f>> {
        self.insert(ptr::null_mut(), ptr::null(), tag, value)
    }
    fn insert(
        &mut self,
        parent: *mut native::tlv_node_t,
        before: *const native::tlv_node_t,
        tag: &Tag,
        value: &[u8],
    ) -> Result<NodeMut<'_, 'f>> {
        let mut raw = ptr::null_mut();
        // SAFETY: callers supply only live nodes from this exclusively borrowed document.
        Error::check(unsafe {
            native::tlv_document_insert(
                self.raw,
                parent,
                before,
                tag.raw(),
                value.as_ptr(),
                value.len(),
                &mut raw,
            )
        })?;
        Ok(NodeMut {
            raw,
            document: self,
        })
    }
    /// Exact encoded size using the document's original Format.
    pub fn encoded_size(&self) -> Result<usize> {
        self.measure(self.format)
    }
    /// Exact encoded size using a compatible destination builtin Format.
    pub fn encoded_size_as(&self, format: Format) -> Result<usize> {
        self.measure(format.raw())
    }
    /// Exact encoded size using a borrowed destination Fixed Format.
    pub fn encoded_size_fixed(&self, format: &FixedFormat<'_>) -> Result<usize> {
        self.measure(format.raw())
    }
    fn measure(&self, format: *const native::tlv_format_t) -> Result<usize> {
        let mut size = 0;
        // SAFETY: immutable document and a valid Format for the synchronous call.
        Error::check(unsafe { native::tlv_document_encoded_size_as(self.raw, format, &mut size) })?;
        Ok(size)
    }
    /// Encode into caller-owned storage; capacity errors carry the required size.
    pub fn encode_into(&self, output: &mut [u8]) -> DocResult<usize> {
        self.write(self.format, output)
    }
    /// Encode using a compatible destination builtin, without modifying the document.
    pub fn encode_into_as(&self, output: &mut [u8], format: Format) -> DocResult<usize> {
        self.write(format.raw(), output)
    }
    /// Encode using a destination Fixed Format into caller-owned storage.
    pub fn encode_into_fixed(
        &self,
        output: &mut [u8],
        format: &FixedFormat<'_>,
    ) -> DocResult<usize> {
        self.write(format.raw(), output)
    }
    fn write(&self, format: *const native::tlv_format_t, output: &mut [u8]) -> DocResult<usize> {
        let mut written = 0;
        // SAFETY: output is exclusive and disjoint from owned document storage.
        let code = unsafe {
            native::tlv_document_encode_as(
                self.raw,
                format,
                output.as_mut_ptr(),
                output.len(),
                &mut written,
            )
        };
        encoding_result(code, written)
    }
    /// Allocate and encode using the original Format.
    pub fn encode(&self) -> DocResult<Vec<u8>> {
        self.encode_format(self.format)
    }
    /// Allocate and encode using a compatible destination builtin Format.
    pub fn encode_as(&self, format: Format) -> DocResult<Vec<u8>> {
        self.encode_format(format.raw())
    }
    fn encode_format(&self, format: *const native::tlv_format_t) -> DocResult<Vec<u8>> {
        let mut output = output_buffer(self.measure(format)?)?;
        let size = self.write(format, &mut output)?;
        output.truncate(size);
        Ok(output)
    }
}
impl Drop for Document<'_> {
    fn drop(&mut self) {
        // SAFETY: sole owner; borrow checking prevents live node references here.
        unsafe { native::tlv_document_free(self.raw) };
    }
}

/// Immutable node borrowing its owning Document. Values are zero-copy views.
#[derive(Clone, Copy)]
pub struct Node<'d, 'f> {
    pub(crate) raw: *mut native::tlv_node_t,
    document: &'d Document<'f>,
}
impl<'d, 'f> Node<'d, 'f> {
    /// Stable native identity scoped to this immutably borrowed Document.
    pub fn identity(&self) -> u64 {
        // SAFETY: immutable Document borrow keeps this node alive.
        unsafe { native::tlv_node_identity(self.raw) }
    }
    /// Copy the node's identifier into an owned Tag.
    pub fn tag(&self) -> Tag {
        // SAFETY: C-owned node Tag is live for the document borrow.
        unsafe { Tag::from_raw(&native::tlv_node_tag(self.raw)) }.expect("C node has a valid tag")
    }
    /// Whether the node owns nested elements.
    pub fn is_constructed(&self) -> bool {
        // SAFETY: immutable borrowed node.
        unsafe { native::tlv_node_is_constructed(self.raw) != 0 }
    }
    /// Borrow primitive Value bytes; constructed nodes return an empty slice.
    pub fn value(&self) -> &'d [u8] {
        // SAFETY: storage is owned by the immutably borrowed document, and cannot
        // be replaced until all returned views expire. C uses NULL for empty Values.
        unsafe {
            let size = native::tlv_node_value_size(self.raw);
            if size == 0 {
                &[]
            } else {
                slice::from_raw_parts(native::tlv_node_value_data(self.raw), size)
            }
        }
    }
    /// First direct child, as selected by C.
    pub fn first_child(&self) -> Option<Self> {
        // SAFETY: live immutable node and owning document.
        self.document
            .node(unsafe { native::tlv_node_first_child(self.raw) })
    }
    /// Next sibling, as selected by C.
    pub fn next(&self) -> Option<Self> {
        // SAFETY: live immutable node and owning document.
        self.document
            .node(unsafe { native::tlv_node_next(self.raw) })
    }
    /// Next sibling with the same Tag, as selected by C.
    pub fn next_same_tag(&self) -> Option<Self> {
        // SAFETY: live immutable node and owning document.
        self.document
            .node(unsafe { native::tlv_node_next_same_tag(self.raw) })
    }
    /// Parent, or None for a root.
    pub fn parent(&self) -> Option<Self> {
        // SAFETY: live immutable node and owning document.
        self.document
            .node(unsafe { native::tlv_node_parent(self.raw) })
    }
    /// Find the first direct child with a Tag using C's lookup.
    pub fn find(&self, tag: &Tag) -> Option<Self> {
        // SAFETY: node belongs to this document; tag is live during lookup.
        self.document
            .node(unsafe { native::tlv_document_find(self.document.raw, self.raw, tag.raw()) })
    }
    /// Exact encoded size of this subtree in the document's original Format.
    pub fn encoded_size(&self) -> Result<usize> {
        self.measure(self.document.format)
    }
    /// Exact encoded size of this subtree in a destination builtin Format.
    pub fn encoded_size_as(&self, format: Format) -> Result<usize> {
        self.measure(format.raw())
    }
    fn measure(&self, format: *const native::tlv_format_t) -> Result<usize> {
        let mut size = 0;
        // SAFETY: immutable node and live destination Format.
        Error::check(unsafe { native::tlv_node_encoded_size_as(self.raw, format, &mut size) })?;
        Ok(size)
    }
    /// Encode this subtree into caller-owned output.
    pub fn encode_into(&self, output: &mut [u8]) -> DocResult<usize> {
        self.write(self.document.format, output)
    }
    /// Encode this subtree into caller-owned output using a destination builtin.
    pub fn encode_into_as(&self, output: &mut [u8], format: Format) -> DocResult<usize> {
        self.write(format.raw(), output)
    }
    fn write(&self, format: *const native::tlv_format_t, output: &mut [u8]) -> DocResult<usize> {
        let mut written = 0;
        // SAFETY: immutable node and disjoint exclusive output.
        let code = unsafe {
            native::tlv_node_encode_as(
                self.raw,
                format,
                output.as_mut_ptr(),
                output.len(),
                &mut written,
            )
        };
        encoding_result(code, written)
    }
    /// Allocate an encoding of this subtree using the original Format.
    pub fn encode(&self) -> DocResult<Vec<u8>> {
        let mut output = output_buffer(self.encoded_size()?)?;
        let size = self.encode_into(&mut output)?;
        output.truncate(size);
        Ok(output)
    }
}

/// Exclusive access to one node and its owning document.
///
/// Navigation consumes the cursor, so no mutable aliases survive. Temporary
/// immutable views borrow the cursor and prevent mutation while retained.
pub struct NodeMut<'d, 'f> {
    raw: *mut native::tlv_node_t,
    document: &'d mut Document<'f>,
}
impl<'d, 'f> NodeMut<'d, 'f> {
    /// Borrow an immutable view without releasing exclusive ownership.
    pub fn as_node(&self) -> Node<'_, 'f> {
        Node {
            raw: self.raw,
            document: self.document,
        }
    }
    /// Replace the Value, parsing children through C for constructed nodes.
    pub fn set_value(&mut self, value: &[u8]) -> Result<()> {
        // SAFETY: exclusive document borrow prevents retained node/value aliases.
        Error::check(unsafe { native::tlv_node_set_value(self.raw, value.as_ptr(), value.len()) })
    }
    /// Erase this node and descendants, consuming the exclusive handle.
    pub fn erase(self) {
        // SAFETY: exclusive document access; no child or value borrows remain.
        unsafe { native::tlv_node_erase(self.raw) };
    }
    /// Append a copied child; constructed Values are parsed by C.
    pub fn append(&mut self, tag: &Tag, value: &[u8]) -> Result<NodeMut<'_, 'f>> {
        self.document.insert(self.raw, ptr::null(), tag, value)
    }
    /// Insert a sibling immediately before this node.
    pub fn insert_before(&mut self, tag: &Tag, value: &[u8]) -> Result<NodeMut<'_, 'f>> {
        // SAFETY: live exclusive node; C returns its owning parent.
        let parent = unsafe { native::tlv_node_parent(self.raw) };
        self.document.insert(parent, self.raw, tag, value)
    }
    /// Move exclusive access to the first direct child.
    pub fn into_first_child(self) -> Option<Self> {
        // SAFETY: exclusive live node; consumes the old handle.
        let raw = unsafe { native::tlv_node_first_child(self.raw) };
        (!raw.is_null()).then_some(Self {
            raw,
            document: self.document,
        })
    }
    /// Move exclusive access to the next sibling.
    pub fn into_next(self) -> Option<Self> {
        // SAFETY: exclusive live node; consumes the old handle.
        let raw = unsafe { native::tlv_node_next(self.raw) };
        (!raw.is_null()).then_some(Self {
            raw,
            document: self.document,
        })
    }
    /// Move exclusive access to the parent.
    pub fn into_parent(self) -> Option<Self> {
        // SAFETY: exclusive live node; consumes the old handle.
        let raw = unsafe { native::tlv_node_parent(self.raw) };
        (!raw.is_null()).then_some(Self {
            raw,
            document: self.document,
        })
    }
}

fn output_buffer(size: usize) -> Result<Vec<u8>> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(size)
        .map_err(|_| Error::OutOfMemory)?;
    result.resize(size, 0);
    Ok(result)
}
fn encoding_result(code: i32, written: usize) -> DocResult<usize> {
    Error::check(code).map_err(|error| DocumentError {
        error,
        offset: None,
        location: crate::Location::default(),
        required: (error == Error::BufferTooShort).then_some(written),
    })?;
    Ok(written)
}
