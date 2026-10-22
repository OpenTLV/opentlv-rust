// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

//! Safe preorder projection of the canonical C Tree Reader.
use crate::{Decoded, Error, FixedFormat, Format, ReaderDiagnostic, Result};
use opentlv_sys as native;
use std::{marker::PhantomData, mem::MaybeUninit};

/// A complete borrowed element with its original source and traversal location.
#[derive(Clone, Debug)]
pub struct TreeItem<'a> {
    /// Content and original encoding.
    pub decoded: Decoded<'a>,
    /// Zero-based nesting depth.
    pub depth: usize,
    /// Absolute encoded-element offset.
    pub offset: usize,
    /// Whether Format classifies the item as constructed.
    pub constructed: bool,
}

/// Canonical structural stream. END has no borrowed parent payload.
#[derive(Clone, Debug)]
pub enum TreeEvent<'a> {
    /// Open a constructed node; Writer ignores its original Value.
    Begin(TreeItem<'a>),
    /// One primitive node.
    Element(TreeItem<'a>),
    /// Close the innermost container; skipped marks omitted descendants.
    End {
        /// Depth of the container being closed.
        depth: usize,
        /// Absolute end of the original Value, before its trailer.
        offset: usize,
        /// Descendants were omitted instead of validated.
        skipped: bool,
    },
}

/// Preorder traversal backed exclusively by C; owns bounded frame storage.
/// Input and custom fixed descriptors are borrowed for `'a`. Moving the reader
/// does not move its heap-backed frames. It cannot be cloned to alias frame state.
pub struct TreeReader<'a> {
    pub(crate) raw: native::tlv_tree_reader_t,
    pub(crate) current: Option<native::tlv_tree_item_t>,
    _frames: Box<[native::tlv_tree_frame_t]>,
    pub(crate) diagnostic: Option<ReaderDiagnostic>,
    input: &'a [u8],
    lifetime: PhantomData<&'a [u8]>,
}

impl<'a> TreeReader<'a> {
    /// Creates a bounded reader; `final_input = false` enables resumable input.
    pub fn new(
        data: &'a [u8],
        format: Format,
        frame_capacity: usize,
        max_depth: usize,
        max_elements: usize,
        final_input: bool,
    ) -> Result<Self> {
        // SAFETY: builtin descriptors have static lifetime.
        unsafe {
            Self::init(
                data,
                format.raw(),
                frame_capacity,
                max_depth,
                max_elements,
                final_input,
            )
        }
    }

    /// Creates traversal borrowing a fixed descriptor and configuration for `'a`.
    pub fn with_fixed_format(
        data: &'a [u8],
        format: &'a FixedFormat<'_>,
        frame_capacity: usize,
        max_depth: usize,
        max_elements: usize,
        final_input: bool,
    ) -> Result<Self> {
        // SAFETY: signature borrows the descriptor and context for 'a.
        unsafe {
            Self::init(
                data,
                format.raw(),
                frame_capacity,
                max_depth,
                max_elements,
                final_input,
            )
        }
    }

    unsafe fn init(
        data: &'a [u8],
        format: *const native::tlv_format_t,
        capacity: usize,
        max_depth: usize,
        max_elements: usize,
        final_input: bool,
    ) -> Result<Self> {
        let mut frames = Vec::new();
        frames
            .try_reserve_exact(capacity)
            .map_err(|_| Error::OutOfMemory)?;
        frames.resize(capacity, native::tlv_tree_frame_t { end: 0, resume: 0 });
        let mut frames = frames.into_boxed_slice();
        let mut raw = MaybeUninit::uninit();
        let init = if final_input {
            native::tlv_tree_reader_init
        } else {
            native::tlv_tree_reader_init_incremental
        };
        // SAFETY: writable cursor and exclusive heap frames; input and descriptor live for 'a.
        Error::check(unsafe {
            init(
                raw.as_mut_ptr(),
                data.as_ptr(),
                data.len(),
                format,
                frames.as_mut_ptr(),
                capacity,
                max_depth,
                max_elements,
            )
        })?;
        Ok(Self {
            // SAFETY: successful init initialized all native state.
            raw: unsafe { raw.assume_init() },
            _frames: frames,
            diagnostic: None,
            current: None,
            input: data,
            lifetime: PhantomData,
        })
    }

    /// Pulls a complete item, preserving NEED_MORE_DATA, EOF and resource errors.
    pub fn read(&mut self) -> Result<TreeItem<'a>> {
        self.current = None;
        let mut item = MaybeUninit::uninit();
        let mut diag = MaybeUninit::uninit();
        // SAFETY: initialized cursor, writable outputs and live input/frames.
        let (code, diag) = unsafe {
            native::tlv_reader_diagnostic_init(diag.as_mut_ptr());
            let code = native::tlv_tree_reader_next_diag(
                &mut self.raw,
                item.as_mut_ptr(),
                diag.as_mut_ptr(),
            );
            (code, diag.assume_init())
        };
        // SAFETY: diagnostic borrowed fields refer to live input/Format storage.
        self.diagnostic =
            (code != native::TLV_OK).then(|| unsafe { ReaderDiagnostic::from_raw(&diag) });
        Error::check(code)?;
        // SAFETY: successful pull initialized item with storage borrowed for 'a.
        let item = unsafe { item.assume_init() };
        let result = TreeItem {
            // SAFETY: complete output of the same successful native decode.
            decoded: unsafe {
                Decoded::from_raw(native::tlv_decoded_t {
                    element: item.element,
                    source: item.source,
                })
            }?,
            depth: item.depth,
            offset: item.offset,
            constructed: item.constructed != 0,
        };
        self.current = Some(item);
        Ok(result)
    }

    /// Pull one canonical event. Do not mix with node-only reads for a balanced stream.
    pub fn read_event(&mut self) -> Result<TreeEvent<'a>> {
        self.current = None;
        let mut event = MaybeUninit::uninit();
        let mut diag = MaybeUninit::uninit();
        // SAFETY: cursor and borrowed input remain live, outputs are writable.
        let (code, diag) = unsafe {
            native::tlv_reader_diagnostic_init(diag.as_mut_ptr());
            let code = native::tlv_tree_reader_next_event_diag(
                &mut self.raw,
                event.as_mut_ptr(),
                diag.as_mut_ptr(),
            );
            (code, diag.assume_init())
        };
        // SAFETY: original diagnostic storage remains live.
        self.diagnostic =
            (code != native::TLV_OK).then(|| unsafe { ReaderDiagnostic::from_raw(&diag) });
        Error::check(code)?;
        // SAFETY: successful pull initialized event.
        let event = unsafe { event.assume_init() };
        if event.kind == native::TLV_TREE_END {
            return Ok(TreeEvent::End {
                depth: event.depth,
                offset: event.offset,
                skipped: event.skipped != 0,
            });
        }
        let item = TreeItem {
            // SAFETY: node event contains a complete decode borrowing input for 'a.
            decoded: unsafe {
                Decoded::from_raw(native::tlv_decoded_t {
                    element: event.element,
                    source: event.source,
                })
            }?,
            depth: event.depth,
            offset: event.offset,
            constructed: event.kind == native::TLV_TREE_BEGIN,
        };
        Ok(if item.constructed {
            TreeEvent::Begin(item)
        } else {
            TreeEvent::Element(item)
        })
    }

    /// Skips the pending subtree; may recover from a descent limit failure.
    pub fn skip_subtree(&mut self) -> Result<()> {
        self.current = None;
        // SAFETY: exclusively borrowed initialized cursor.
        Error::check(unsafe { native::tlv_tree_reader_skip_subtree(&mut self.raw) })
    }
    /// Replaces the window, retaining undiscarded bytes unchanged and all old borrows.
    pub fn set_input(&mut self, data: &'a [u8], discard: usize, final_input: bool) -> Result<()> {
        self.current = None;
        if discard > self.input.len() || !data.starts_with(&self.input[discard..]) {
            return Err(Error::InvalidArg);
        }
        // SAFETY: new immutable input lives for 'a; frames remain owned by self.
        Error::check(unsafe {
            native::tlv_tree_reader_set_input(
                &mut self.raw,
                data.as_ptr(),
                data.len(),
                discard,
                final_input.into(),
            )
        })?;
        self.input = data;
        Ok(())
    }
    /// Parser-discardable bytes in the current window.
    pub fn consumed(&self) -> usize {
        // SAFETY: initialized cursor.
        unsafe { native::tlv_tree_reader_consumed(&self.raw) }
    }
    /// Absolute traversal frontier.
    pub fn offset(&self) -> usize {
        // SAFETY: initialized cursor.
        unsafe { native::tlv_tree_reader_offset(&self.raw) }
    }
    /// True only at final exhaustion, never when more input is needed.
    pub fn is_at_end(&self) -> bool {
        // SAFETY: initialized cursor.
        unsafe { native::tlv_tree_reader_at_end(&self.raw) != 0 }
    }
    /// Owned detail from the last failed pull, cleared by success.
    pub fn diagnostic(&self) -> Option<&ReaderDiagnostic> {
        self.diagnostic.as_ref()
    }

    /// Visit preorder items through C, passing Element, depth and absolute offset.
    /// STOP leaves the current item published; resume or skip its pending subtree.
    /// Panics resume after C returns, with callback effects and cursor progress retained.
    pub fn visit(
        &mut self,
        callback: impl FnMut(crate::Element<'a>, usize, usize) -> crate::Visit,
    ) -> Result<()> {
        self.current = None;
        // SAFETY: exclusive cursor and frames, input and Format borrowed for 'a.
        let (result, diagnostic) = unsafe {
            crate::visitor::run(
                std::ptr::null_mut(),
                &mut self.raw,
                callback,
                std::ptr::null_mut(),
            )
        };
        self.diagnostic = diagnostic;
        result
    }
}

impl<'a> Iterator for TreeReader<'a> {
    type Item = Result<TreeItem<'a>>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.is_at_end() {
            self.current = None;
            None
        } else {
            Some(self.read())
        }
    }
}
