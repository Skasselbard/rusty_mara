// #![cfg_attr(feature = "bit", feature(bit))]

use crate::code_block;
use crate::globals::*;
use crate::space::*;
use crate::AllocationData;

/// #### return
/// a pointer to the next FreeSpace in the bucket. The left next pointer will be taken as reference.
/// Next pointer are an offset of the page start
/// nullptr if the offset == 0
#[inline]
pub unsafe fn get_next(alloc_data: &AllocationData) -> *mut NextPointerType {
    let left_next = generate_next_pointer_position(alloc_data);
    if *left_next == ERROR_NEXT_POINTER {
        core::ptr::null_mut()
    } else {
        (alloc_data
            .calculate_start_of_page()
            .add(*left_next as usize)) as *mut NextPointerType
    }
}

/// Adapt the next pointer in the data structure. The next pointer is adjacent to the
/// code blocks and stored as a 4 byte integer interpreted as offset from the last byte
/// (to the right)
/// #### next
/// pointer to the next free space. Can be null. If null the offset is set to 0 which
/// will be interpreted as if there is no successor.
/// #### start_of_page
/// the start of the page the space is in. Is needed to calculate the offset that is
/// actually saved in the space
pub unsafe fn set_next(alloc_data: &mut AllocationData, next: *mut NextPointerType) {
    #[cfg(feature = "consistency-checks")]
    {
        assert!(next.is_null() || next as *const u8 >= alloc_data.calculate_start_of_page());
        assert!(
            (next as usize).saturating_sub(alloc_data.calculate_start_of_page() as usize)
                < NextPointerType::max_value() as usize
        );
        // offset is less than uint 32
    }
    let alloc_next = generate_next_pointer_position(alloc_data);
    if next == core::ptr::null_mut() {
        *alloc_next = ERROR_NEXT_POINTER;
        #[cfg(feature = "consistency-checks")]
        {}
        return;
    }
    *alloc_next = (next.sub(alloc_data.calculate_start_of_page() as usize)) as NextPointerType;

    alloc_data.check_next_boundaries();
    #[cfg(feature = "consistency-checks")]
    {
        assert!(*alloc_next != ERROR_NEXT_POINTER);
    }
}
/// Write a code block that is consistent with the allocation size (``data_start``
/// to ``data_end``).
/// The code block is copied to the end of the allocation and the free bit is
/// determined by ``is_free``.
/// Allocation cache for ``space`` and ``space size is updated``
/// The ``next`` pointer is also written at the correct location
pub unsafe fn write_data_size_code_blocks(alloc_data: &mut AllocationData, is_free: bool) {
    let code_block_size = code_block::generate_code_block_for_internal_size(
        alloc_data.data_start(),
        alloc_data.calculate_data_size(),
        is_free,
    );
    alloc_data.set_code_block_size(code_block_size);
    copy_code_block_to_end(alloc_data);
    // update allocation
    alloc_data.set_space_size(alloc_data.calculate_data_size() - 2 * code_block_size);
    alloc_data.set_space(alloc_data.data_start().add(code_block_size));
    write_next_pointer(alloc_data);
    #[cfg(feature = "consistency-checks")]
    {
        let (right_block_size, _) = code_block::read_from_right(alloc_data.data_end());
        assert!(code_block::read_from_left(alloc_data.data_start()) == right_block_size,);
    }
}
/// Write a code block that is consistent with the ``space_size`` of an allocation.
/// The code block is copied to the end of the allocation and the free bit is
/// determined by ``is_free``.
/// Allocation cache for ``data_end`` and ``space`` is updated (allocation might
/// shrink if the code block get smaller)
pub unsafe fn write_space_size_code_blocks(alloc_data: &mut AllocationData, is_free: bool) {
    code_block::generate_code_block_for_payload_size(alloc_data, is_free);
    alloc_data.set_space(alloc_data.data_start().add(alloc_data.code_block_size()));
    alloc_data.set_data_end(
        alloc_data
            .space()
            .add(alloc_data.space_size())
            .add(alloc_data.code_block_size())
            .sub(1),
    );
    copy_code_block_to_end(alloc_data);
    #[cfg(feature = "consistency-checks")]
    {
        let (right_block_size, _) = code_block::read_from_right(alloc_data.data_end());
        assert!(code_block::read_from_left(alloc_data.data_start()) == right_block_size,);
    }
}
/// sets the next pointer cache in the alloc data to the value of the left next pointer
/// in the data array
#[inline]
pub fn generate_next_pointer_position(alloc_data: &AllocationData) -> *mut NextPointerType {
    if alloc_data.space_is_init() {
        alloc_data.space() as *mut NextPointerType
    } else {
        unsafe { alloc_data.data_start().add(alloc_data.code_block_size()) as *mut NextPointerType }
    }
}
#[inline]
pub fn write_next_pointer(alloc_data: &AllocationData) {
    unsafe {
        *generate_next_pointer_position(alloc_data) = match alloc_data.next_pointer().is_null() {
            true => ERROR_NEXT_POINTER,
            false => alloc_data.next_pointer() as NextPointerType,
        };
    }
}
