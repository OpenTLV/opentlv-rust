// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Marek Cingel

use opentlv::Error;
use opentlv_sys as native;
use std::mem::MaybeUninit;

#[test]
fn native_reader_layout_and_incremental_transitions_match_c() {
    let config = native::tlv_fixed_format_t {
        identifier: native::tlv_fixed_identifier_t { size: 1 },
        length: native::tlv_fixed_length_t {
            size: 1,
            byte_order: native::TLV_BYTE_ORDER_BIG_ENDIAN,
        },
        element_order: native::TLV_ELEMENT_ORDER_TLV,
        length_scope: native::TLV_LENGTH_SCOPE_VALUE,
    };
    let first = [1u8, 0, 2, 1];
    let second = [2u8, 1, 0xAA];
    let mut format = MaybeUninit::<native::tlv_format_t>::uninit();
    let mut reader = MaybeUninit::<native::tlv_reader_t>::uninit();
    let mut element = MaybeUninit::<native::tlv_element_t>::uninit();
    // SAFETY: Every buffer and the immutable Format/context outlive the cursor
    // and every borrowed element; each output is read only after C succeeds.
    unsafe {
        assert_eq!(
            native::tlv_fixed_format_init(format.as_mut_ptr(), &config),
            native::TLV_OK
        );
        let format = format.assume_init();
        assert_eq!(
            native::tlv_reader_init_incremental(
                reader.as_mut_ptr(),
                first.as_ptr(),
                first.len(),
                &format
            ),
            native::TLV_OK
        );
        let mut reader = reader.assume_init();
        assert_eq!(reader.base_offset, 0);
        assert_eq!(reader.final_input, 0);
        assert_eq!(
            native::tlv_reader_next(&mut reader, element.as_mut_ptr()),
            native::TLV_OK
        );
        assert_eq!(
            native::tlv_reader_next(&mut reader, element.as_mut_ptr()),
            native::TLV_NEED_MORE_DATA
        );
        assert_eq!(native::tlv_reader_consumed(&reader), 2);
        assert_eq!(
            native::tlv_reader_set_input(&mut reader, second.as_ptr(), second.len(), 2, 1),
            native::TLV_OK
        );
        assert_eq!(native::tlv_reader_offset(&reader), 2);
        assert_eq!(
            native::tlv_reader_next(&mut reader, element.as_mut_ptr()),
            native::TLV_OK
        );
        assert_eq!(element.assume_init().value.data, second.as_ptr().add(2));
        assert_eq!(native::tlv_reader_offset(&reader), 5);
        assert_eq!(native::tlv_reader_at_end(&reader), 1);
        assert_eq!(
            native::tlv_reader_next(&mut reader, element.as_mut_ptr()),
            native::TLV_END
        );
    }
    assert_eq!(
        Error::from_code(native::TLV_NEED_MORE_DATA),
        Some(Error::NeedMoreData)
    );
    assert_eq!(Error::NeedMoreData.code(), native::TLV_NEED_MORE_DATA);
}
