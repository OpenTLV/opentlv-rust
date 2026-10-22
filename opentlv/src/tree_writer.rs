// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Bounded tree output through the canonical C Tree Writer.
use crate::{Element, Error, FixedFormat, Format, Result, Tag};
use opentlv_sys as native;
use std::{
    any::Any,
    ffi::c_void,
    panic::{catch_unwind, resume_unwind, AssertUnwindSafe},
};
use std::{ffi::CStr, marker::PhantomData, mem::MaybeUninit, ptr, slice};

/// One semantic preorder record supplied to exact tree measurement.
pub struct TreeWriteItem<'a> {
    /// Semantic content; constructed Values are ignored by C measurement.
    pub element: Element<'a>,
    /// Root-relative depth, starting at zero.
    pub depth: usize,
    /// Whether this record opens a parent, including an empty parent.
    pub constructed: bool,
}

struct MeasureSource<'a, I> {
    items: I,
    retained: Vec<TreeWriteItem<'a>>,
    error: Option<Error>,
    panic: Option<Box<dyn Any + Send>>,
}

unsafe extern "C" fn measure_next<'a, I: Iterator<Item = Result<TreeWriteItem<'a>>>>(
    context: *mut c_void,
    element: *mut native::tlv_element_t,
    depth: *mut usize,
    constructed: *mut i32,
) -> i32 {
    // SAFETY: measure supplies this exact live source and C supplies writable outputs.
    let source = unsafe { &mut *context.cast::<MeasureSource<'a, I>>() };
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let Some(item) = source.items.next() else {
            return Ok(native::TLV_END);
        };
        let item = item?;
        source
            .retained
            .try_reserve(1)
            .map_err(|_| Error::OutOfMemory)?;
        source.retained.push(item);
        let item = source.retained.last().unwrap();
        // SAFETY: retained Tags own stable heap bytes, even if the record Vec moves.
        // Primitive Values borrow immutable input for the duration of measurement.
        unsafe {
            *element = native::tlv_element_t {
                tag: item.element.tag().raw(),
                value: native::tlv_value_t {
                    data: item.element.value().as_ptr(),
                    size: item.element.value().len() as u64,
                },
            };
            *depth = item.depth;
            *constructed = i32::from(item.constructed);
        }
        Ok(native::TLV_OK)
    }));
    match outcome {
        Ok(Ok(code)) => code,
        Ok(Err(error)) => {
            source.error = Some(error);
            native::TLV_ERR_INVALID_ARG
        }
        Err(panic) => {
            source.panic = Some(panic);
            native::TLV_ERR_INVALID_ARG
        }
    }
}

struct EventMeasureSource<'a, I> {
    items: I,
    retained: Vec<crate::TreeEvent<'a>>,
    error: Option<Error>,
    panic: Option<Box<dyn Any + Send>>,
}

unsafe extern "C" fn measure_event_next<'a, I: Iterator<Item = Result<crate::TreeEvent<'a>>>>(
    context: *mut c_void,
    event: *mut native::tlv_tree_event_t,
) -> i32 {
    // SAFETY: synchronous measurement supplies this exact context and writable event.
    let source = unsafe { &mut *context.cast::<EventMeasureSource<'a, I>>() };
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let Some(item) = source.items.next() else {
            return Ok(native::TLV_END);
        };
        let item = item?;
        source
            .retained
            .try_reserve(1)
            .map_err(|_| Error::OutOfMemory)?;
        source.retained.push(item);
        // SAFETY: native record has nullable pointers and scalar fields only.
        let mut raw: native::tlv_tree_event_t = unsafe { std::mem::zeroed() };
        match source.retained.last().unwrap() {
            crate::TreeEvent::Begin(item) | crate::TreeEvent::Element(item) => {
                raw.kind = if matches!(source.retained.last(), Some(crate::TreeEvent::Begin(_))) {
                    native::TLV_TREE_BEGIN
                } else {
                    native::TLV_TREE_ELEMENT
                };
                raw.depth = item.depth;
                raw.element.tag = item.decoded.element.tag().raw();
                raw.element.value = native::tlv_value_t {
                    data: item.decoded.element.value().as_ptr(),
                    size: item.decoded.element.value().len() as u64,
                };
            }
            crate::TreeEvent::End {
                depth,
                offset,
                skipped,
            } => {
                raw.kind = native::TLV_TREE_END;
                raw.depth = *depth;
                raw.offset = *offset;
                raw.skipped = i32::from(*skipped);
            }
        }
        // SAFETY: output is writable and retained Tags own stable storage.
        unsafe {
            *event = raw;
        }
        Ok(native::TLV_OK)
    }));
    match outcome {
        Ok(Ok(code)) => code,
        Ok(Err(error)) => {
            source.error = Some(error);
            native::TLV_ERR_INVALID_ARG
        }
        Err(panic) => {
            source.panic = Some(panic);
            native::TLV_ERR_INVALID_ARG
        }
    }
}

/// Owned details for a failed Writer operation.
#[derive(Clone, Debug)]
pub struct WriterDiagnostic {
    /// Canonical diagnostic severity.
    pub severity: crate::Severity,
    /// Owned subsystem context entries.
    pub contexts: Vec<crate::DiagnosticContext>,
    /// Retained enclosing tags; None means no path evidence.
    pub path: Option<Vec<Vec<u8>>>,
    /// Number of omitted innermost scopes.
    pub path_omitted: usize,

    /// Absolute failing output offset when available.
    pub offset: Option<usize>,
    /// C Writer operation code.
    pub operation: crate::WriterOperation,
    /// Primary output evidence coordinates.
    pub location: crate::Location,
    /// Tag being encoded, copied before temporary input expires.
    pub tag: Option<Tag>,
    /// Native Value size, when representable.
    pub length: Option<usize>,
    /// Total encoded capacity required.
    pub required: Option<usize>,
    /// Available output bytes.
    pub available: Option<usize>,
    /// Expected description reported by the Format.
    pub expected: Option<String>,
    /// Actual description reported by the Format.
    pub actual: Option<String>,
}

impl WriterDiagnostic {
    /// Copy native details while their borrowed fields remain alive.
    pub(crate) unsafe fn from_raw(diag: &native::tlv_writer_diagnostic_t) -> Self {
        let text = |p: *const std::os::raw::c_char| {
            if p.is_null() {
                None
            } else {
                // SAFETY: C descriptions are live NUL-terminated strings.
                Some(unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
            }
        };
        Self {
            offset: (diag.diagnostic.location.kind != 0).then_some(diag.diagnostic.location.begin),
            operation: crate::WriterOperation::from_raw(diag.operation),
            severity: crate::Severity::from_raw(diag.diagnostic.severity),
            contexts: unsafe { crate::reader_diagnostic::contexts(&diag.diagnostic) },
            path: unsafe { crate::reader_diagnostic::path(&diag.diagnostic) },
            path_omitted: diag.diagnostic.path.omitted,
            location: crate::Location::from_raw(diag.diagnostic.location),
            // SAFETY: diagnostic tag is copied while its operation inputs remain live.
            tag: if diag.has_tag != 0 {
                unsafe { Tag::from_raw(&diag.tag) }.ok()
            } else {
                None
            },
            length: (diag.has_length != 0).then_some(diag.length),
            required: (diag.has_required != 0).then_some(diag.required),
            available: (diag.has_available != 0).then_some(diag.available),
            expected: text(diag.diagnostic.expected),
            actual: text(diag.diagnostic.actual),
        }
    }
}

/// Tree Writer borrowing output and owning bounded frames, scratch and open tags.
/// Successful begin retains an owned tag until end. Output never grows implicitly.
pub struct TreeWriter<'a> {
    raw: native::tlv_tree_writer_t,
    output: PhantomData<&'a mut [u8]>,
    _frames: Box<[native::tlv_tree_writer_frame_t]>,
    _scratch: Box<[u8]>,
    tags: Vec<Tag>,
    tag_storage: Option<Box<[u8]>>,
    diagnostic: Option<WriterDiagnostic>,
    required_workspace: (usize, usize),
}

impl<'a> TreeWriter<'a> {
    /// Initialize bounded output with explicit frame, scratch, depth and item limits.
    pub fn new(
        output: &'a mut [u8],
        format: Format,
        frame_capacity: usize,
        scratch_capacity: usize,
        max_depth: usize,
        max_elements: usize,
    ) -> Result<Self> {
        // SAFETY: static builtin descriptor.
        unsafe {
            Self::init(
                output,
                format.raw(),
                frame_capacity,
                scratch_capacity,
                max_depth,
                max_elements,
            )
        }
    }
    /// Initialize while borrowing a Fixed descriptor and configuration for `'a`.
    pub fn with_fixed_format(
        output: &'a mut [u8],
        format: &'a FixedFormat<'_>,
        frame_capacity: usize,
        scratch_capacity: usize,
        max_depth: usize,
        max_elements: usize,
    ) -> Result<Self> {
        // SAFETY: signature keeps the descriptor/context alive with the output borrow.
        unsafe {
            Self::init(
                output,
                format.raw(),
                frame_capacity,
                scratch_capacity,
                max_depth,
                max_elements,
            )
        }
    }
    unsafe fn init(
        output: &'a mut [u8],
        format: *const native::tlv_format_t,
        capacity: usize,
        scratch_capacity: usize,
        max_depth: usize,
        max_elements: usize,
    ) -> Result<Self> {
        let mut frames = Vec::new();
        frames
            .try_reserve_exact(capacity)
            .map_err(|_| Error::OutOfMemory)?;
        frames.resize(
            capacity,
            native::tlv_tree_writer_frame_t {
                tag: native::tlv_tag_t {
                    data: ptr::null(),
                    size: 0,
                },
                start: 0,
            },
        );
        let mut frames = frames.into_boxed_slice();
        let mut scratch = Vec::new();
        scratch
            .try_reserve_exact(scratch_capacity)
            .map_err(|_| Error::OutOfMemory)?;
        scratch.resize(scratch_capacity, 0);
        let mut scratch = scratch.into_boxed_slice();
        let mut tags = Vec::new();
        tags.try_reserve_exact(capacity)
            .map_err(|_| Error::OutOfMemory)?;
        let mut raw = MaybeUninit::uninit();
        // SAFETY: all storage is disjoint and stable; output is exclusively borrowed.
        Error::check(unsafe {
            native::tlv_tree_writer_init(
                raw.as_mut_ptr(),
                output.as_mut_ptr(),
                output.len(),
                format,
                frames.as_mut_ptr(),
                capacity,
                scratch.as_mut_ptr(),
                scratch_capacity,
                max_depth,
                max_elements,
            )
        })?;
        Ok(Self {
            // SAFETY: initialization succeeded.
            raw: unsafe { raw.assume_init() },
            output: PhantomData,
            _frames: frames,
            _scratch: scratch,
            tags,
            tag_storage: None,
            diagnostic: None,
            required_workspace: (0, 0),
        })
    }
    fn operation(
        &mut self,
        call: impl FnOnce(*mut native::tlv_tree_writer_t, *mut native::tlv_writer_diagnostic_t) -> i32,
    ) -> Result<()> {
        let mut diag = MaybeUninit::uninit();
        // SAFETY: initializes every field before use by the synchronous operation.
        unsafe { native::tlv_writer_diagnostic_init(diag.as_mut_ptr()) };
        let code = call(&mut self.raw, diag.as_mut_ptr());
        // SAFETY: diagnostic always initialized; caller retains any input tag/value until return.
        let diag = unsafe { diag.assume_init() };
        // SAFETY: input and retained open tags are still alive.
        self.diagnostic =
            (code != native::TLV_OK).then(|| unsafe { WriterDiagnostic::from_raw(&diag) });
        Error::check(code)
    }
    /// Configure a bounded owned Tag arena with no open parents; None restores borrowing.
    pub fn set_tag_capacity(&mut self, capacity: Option<usize>) -> Result<()> {
        let mut storage = if let Some(capacity) = capacity {
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(capacity.max(1))
                .map_err(|_| Error::OutOfMemory)?;
            bytes.resize(capacity.max(1), 0);
            Some(bytes.into_boxed_slice())
        } else {
            None
        };
        let data = storage
            .as_mut()
            .map_or(ptr::null_mut(), |bytes| bytes.as_mut_ptr());
        // SAFETY: arena is disjoint, owned, and retained after successful configuration.
        Error::check(unsafe {
            native::tlv_tree_writer_set_tag_storage(&mut self.raw, data, capacity.unwrap_or(0))
        })?;
        self.tag_storage = storage;
        Ok(())
    }

    /// Open a constructed item, retaining an owned copy of its tag until successful end.
    pub fn begin(&mut self, tag: &Tag) -> Result<()> {
        let owned = tag.clone();
        // SAFETY: owned tag stays alive through the call and is retained on success.
        self.operation(|writer, diag| unsafe {
            native::tlv_tree_writer_begin_diag(writer, owned.raw(), diag)
        })?;
        self.tags.push(owned);
        Ok(())
    }
    /// Write a primitive or already encoded subtree through the C element writer.
    pub fn write(&mut self, element: &Element<'_>) -> Result<()> {
        let raw = native::tlv_element_t {
            tag: element.tag().raw(),
            value: native::tlv_value_t {
                data: element.value().as_ptr(),
                size: element.value().len() as u64,
            },
        };
        // SAFETY: immutable input cannot alias the exclusively borrowed output buffer.
        self.operation(|writer, diag| unsafe {
            native::tlv_tree_writer_write_element_diag(writer, &raw, diag)
        })
    }
    /// Consume a canonical Reader event through C, retaining an owned BEGIN Tag.
    pub fn write_event(&mut self, event: &crate::TreeEvent<'_>) -> Result<()> {
        // SAFETY: C record consists of scalar fields and nullable pointers.
        let mut raw: native::tlv_tree_event_t = unsafe { std::mem::zeroed() };
        let mut owned = None;
        match event {
            crate::TreeEvent::Begin(item) | crate::TreeEvent::Element(item) => {
                let begin = matches!(event, crate::TreeEvent::Begin(_));
                raw.kind = if begin {
                    native::TLV_TREE_BEGIN
                } else {
                    native::TLV_TREE_ELEMENT
                };
                raw.depth = item.depth;
                raw.offset = item.offset;
                if begin {
                    owned = Some(item.decoded.element.tag().clone());
                }
                raw.element.tag = owned.as_ref().unwrap_or(item.decoded.element.tag()).raw();
                raw.element.value = native::tlv_value_t {
                    data: item.decoded.element.value().as_ptr(),
                    size: item.decoded.element.value().len() as u64,
                };
            }
            crate::TreeEvent::End {
                depth,
                offset,
                skipped,
            } => {
                raw.kind = native::TLV_TREE_END;
                raw.depth = *depth;
                raw.offset = *offset;
                raw.skipped = i32::from(*skipped);
            }
        }
        // SAFETY: input remains live; owned BEGIN Tag is retained on success.
        self.operation(|writer, diag| unsafe {
            native::tlv_tree_writer_write_event_diag(writer, &raw, diag)
        })?;
        if let Some(tag) = owned {
            self.tags.push(tag);
        }
        if raw.kind == native::TLV_TREE_END {
            self.tags.pop();
        }
        Ok(())
    }

    /// Close the innermost parent; failure retains state and open tag for retry.
    pub fn end(&mut self) -> Result<()> {
        // SAFETY: cursor storage and open tags remain alive.
        self.operation(|writer, diag| unsafe { native::tlv_tree_writer_end_diag(writer, diag) })?;
        self.tags.pop();
        Ok(())
    }
    /// Finalized prefix only; unfinished roots remain provisional.
    pub fn written(&self) -> &[u8] {
        // SAFETY: initialized cursor reports a prefix within the borrowed output.
        let size = unsafe { native::tlv_tree_writer_size(&self.raw) };
        // SAFETY: output was an exclusive non-null Rust slice and remains borrowed by self.
        unsafe { slice::from_raw_parts(self.raw.output.buf, size) }
    }
    /// Require all parents closed and return finalized output without sealing the cursor.
    pub fn finish(&self) -> Result<&[u8]> {
        // SAFETY: initialized cursor, immutable check.
        Error::check(unsafe { native::tlv_tree_writer_finish(&self.raw) })?;
        Ok(self.written())
    }
    /// Structured detail for the most recent failed begin/write/end operation.
    pub fn diagnostic(&self) -> Option<&WriterDiagnostic> {
        self.diagnostic.as_ref()
    }

    /// Consume a preorder source through C and return the exact staged encoding.
    ///
    /// Requires an unused writer. The returned slice length is the measurement;
    /// `written()` remains the sequential finalized prefix, which is still empty.
    /// Source items and their Tags are retained until C returns. Source errors and
    /// panics propagate after the C boundary. Retry needs a fresh source.
    pub fn measure<'s, I>(&mut self, items: I) -> Result<&[u8]>
    where
        I: IntoIterator<Item = Result<TreeWriteItem<'s>>>,
    {
        self.required_workspace = (0, 0);
        self.diagnostic = None;
        if self.raw.count != 0 {
            return Err(Error::InvalidArg);
        }
        let mut source = MeasureSource {
            items: items.into_iter(),
            retained: Vec::new(),
            error: None,
            panic: None,
        };
        let mut workspace = native::tlv_tree_writer_workspace_t {
            frames: self.raw.frames,
            frame_capacity: self.raw.capacity,
            data: self.raw.output.buf,
            data_capacity: self.raw.output.capacity,
            scratch: self.raw.scratch,
            scratch_capacity: self.raw.scratch_capacity,
            required_data: 0,
            required_scratch: 0,
        };
        let mut size = 0;
        let mut diag = MaybeUninit::uninit();
        // SAFETY: exclusive self borrow protects disjoint storage. Format and
        // source bytes remain live through the synchronous call and diagnostic copy.
        let code = unsafe {
            native::tlv_writer_diagnostic_init(diag.as_mut_ptr());
            native::tlv_tree_writer_measure(
                self.raw.output.format,
                Some(measure_next::<I::IntoIter>),
                (&mut source as *mut MeasureSource<'s, I::IntoIter>).cast(),
                &mut workspace,
                self.raw.max_depth,
                self.raw.max_elements,
                &mut size,
                diag.as_mut_ptr(),
            )
        };
        self.required_workspace = (workspace.required_data, workspace.required_scratch);
        // SAFETY: initialized diagnostic; source records still retain every borrowed Tag.
        let diag = unsafe { diag.assume_init() };
        self.diagnostic =
            (code != native::TLV_OK).then(|| unsafe { WriterDiagnostic::from_raw(&diag) });
        if let Some(panic) = source.panic {
            resume_unwind(panic);
        }
        if let Some(error) = source.error {
            return Err(error);
        }
        Error::check(code)?;
        // SAFETY: C returned a valid staged prefix within exclusive output storage.
        Ok(unsafe { slice::from_raw_parts(self.raw.output.buf, size) })
    }

    /// Measure a balanced event source through C; requires an unused writer.
    /// Retains event Tags, propagates errors/panics, and reports required workspace.
    pub fn measure_events<'s, I>(&mut self, items: I) -> Result<&[u8]>
    where
        I: IntoIterator<Item = Result<crate::TreeEvent<'s>>>,
    {
        self.required_workspace = (0, 0);
        self.diagnostic = None;
        if self.raw.count != 0 {
            return Err(Error::InvalidArg);
        }
        let mut source = EventMeasureSource {
            items: items.into_iter(),
            retained: Vec::new(),
            error: None,
            panic: None,
        };
        let mut workspace = native::tlv_tree_writer_workspace_t {
            frames: self.raw.frames,
            frame_capacity: self.raw.capacity,
            data: self.raw.output.buf,
            data_capacity: self.raw.output.capacity,
            scratch: self.raw.scratch,
            scratch_capacity: self.raw.scratch_capacity,
            required_data: 0,
            required_scratch: 0,
        };
        let mut size = 0;
        let mut diag = MaybeUninit::uninit();
        // SAFETY: exclusive self borrow protects disjoint storage. Format and
        // source bytes remain live through the synchronous call and diagnostic copy.
        let code = unsafe {
            native::tlv_writer_diagnostic_init(diag.as_mut_ptr());
            native::tlv_tree_writer_measure_events(
                self.raw.output.format,
                Some(measure_event_next::<I::IntoIter>),
                (&mut source as *mut EventMeasureSource<'s, I::IntoIter>).cast(),
                &mut workspace,
                self.raw.max_depth,
                self.raw.max_elements,
                &mut size,
                diag.as_mut_ptr(),
            )
        };
        self.required_workspace = (workspace.required_data, workspace.required_scratch);
        // SAFETY: initialized diagnostic; source records still retain every borrowed Tag.
        let diag = unsafe { diag.assume_init() };
        self.diagnostic =
            (code != native::TLV_OK).then(|| unsafe { WriterDiagnostic::from_raw(&diag) });
        if let Some(panic) = source.panic {
            resume_unwind(panic);
        }
        if let Some(error) = source.error {
            return Err(error);
        }
        Error::check(code)?;
        // SAFETY: C returned a valid staged prefix within exclusive output storage.
        Ok(unsafe { slice::from_raw_parts(self.raw.output.buf, size) })
    }

    /// Last measurement's required (output, scratch) capacities on exhaustion.
    /// These are discovered lower bounds; zero means no such storage shortage.
    pub fn required_workspace(&self) -> (usize, usize) {
        self.required_workspace
    }
}
