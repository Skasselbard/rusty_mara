// #![cfg_attr(feature = "bit", feature(bit))]

use crate::code_block;
use crate::space::*;
use core::mem::size_of;

#[cfg(feature = "bit32")]
type NextPointerType = u32;
#[cfg(feature = "bit16")]
type NextPointerType = u16;
#[cfg(feature = "bit8")]
type NextPointerType = u8;

const ERROR_NEXT_POINTER: NextPointerType = NextPointerType::max_value(); // just ones

/// #### return
/// a pointer to the next FreeSpace in the bucket. The left next pointer will be taken as reference.
/// nullptr if the offset == 0
#[inline]
pub unsafe fn get_next(left_most_end: *const u8, start_of_page: *const u8) -> *mut u8 {
    let code_block_size = code_block::get_block_size(left_most_end);
    let left_next = get_left_next(left_most_end, code_block_size);
    if *left_next == ERROR_NEXT_POINTER {
        return core::ptr::null_mut();
    }
    (start_of_page.offset(*left_next as isize)) as *mut u8
}

/// Adapt the next pointer in the data structure. The next pointer is adjacent to the code blocks and stored
/// as a 4 byte integer interpreted as offset from the last byte (to the right)
/// #### next
/// pointer to the next free space. Can be null. If null the offset is set to 0 which will be interpreted
/// as if there is no successor.
/// #### start_of_page
/// the start of the page the space is in. Is needed to calculate the offset that is actually
/// saved in the space
pub unsafe fn set_next(left_most_end: *const u8, next: *const u8, start_of_page: *const u8) {
    #[cfg(feature = "condition")]
    {
        assert!(next.is_null() || next as *const u8 >= start_of_page);
        assert!((next as usize - start_of_page as usize) < 4294967295); // offset is less than uint 32
    }
    let left_most_end = left_most_end;
    let code_block_size = code_block::get_block_size(left_most_end);
    let left_next = get_left_next(left_most_end, code_block_size);
    let right_next = get_right_next(left_most_end, code_block_size);
    if next == core::ptr::null() {
        *left_next = ERROR_NEXT_POINTER;
        *right_next = ERROR_NEXT_POINTER;
        #[cfg(feature = "condition")]
        {}
        return;
    }
    let offset = (next as usize - start_of_page as usize) as NextPointerType;
    *left_next = offset;
    if code_block::read_from_left(left_most_end) >= 8 {
        //overlapping pointers if the size is too little
        *right_next = offset;
    }
    #[cfg(feature = "condition")]
    {
        assert!(get_next(left_most_end, start_of_page) as *const u8 >= start_of_page);
        assert!(*left_next != ERROR_NEXT_POINTER);
        assert!(*right_next != ERROR_NEXT_POINTER);
    }
}

/// Adjust the size of the free space.<br/>
/// The right most end will be the same as before. The left most end will be the given byte. The next pointer
/// from the free space will be copied from the right to the left
/// #### first_byte
/// the new first byte of the space
/// #### return
/// a pointer to the left most byte of the free space (should be the same as the input)
pub unsafe fn push_beginning_right(left_most_end: *const u8, first_byte: *mut u8) -> *mut u8 {
    #[cfg(feature = "condition")]
    {
        assert!(
            first_byte > left_most_end as *mut u8
                && first_byte < get_right_most_end(left_most_end) as *mut u8
        );
        assert!(first_byte < get_right_most_end(left_most_end) as *mut u8); //Never cross the pointers!
    }
    let code_block_size = code_block::get_block_size(left_most_end);
    let right_most_end = get_right_most_end(left_most_end);
    let next_pointer = *get_right_next(left_most_end, code_block_size);
    let (_, block) = code_block::get_code_block_for_internal_size(
        first_byte,
        (right_most_end as usize - first_byte as usize) + 1,
        true,
    );
    if first_byte == block {
        copy_code_block_to_end(first_byte, code_block_size);
        write_next_pointer(next_pointer, first_byte);
        #[cfg(feature = "condition")]
        {
            let new_free_space: *const u8 = first_byte;
            let (right_block_size, _) = code_block::read_from_right(right_most_end);
            assert!(code_block::read_from_left(first_byte) == right_block_size,);
            assert!(
                code_block::read_from_left(first_byte) < 8
                    || *(get_left_next(new_free_space, code_block::get_block_size(first_byte)))
                        == *(get_right_next(new_free_space, code_block::get_block_size(first_byte))),
            );
        }
        return first_byte;
    } else {
        #[cfg(feature = "condition")]
        {
            assert!(false);
        }
        core::ptr::null_mut()
    }
}
/// Adjust the size of the free space.<br/>
/// The left most end will be the same as before. The right most end will be the given byte. The next pointer
/// from the free space will be copied from the left to the right
/// #### last_byte
/// the new last byte of the space
pub unsafe fn push_end_left(left_most_end: *const u8, last_byte: *const u8) {
    #[cfg(feature = "condition")]
    {
        assert!(last_byte > left_most_end); //the new last byte must be in the old freespace
        assert!(last_byte <= get_right_most_end(left_most_end)); //see above
    }
    let code_block_size = code_block::get_block_size(left_most_end);
    let current_next = *get_left_next(left_most_end, code_block_size); //Needed incase the new CodeBlocks are smaller
    code_block::get_code_block_for_internal_size(
        left_most_end as *mut u8,
        (last_byte as usize - left_most_end as usize) + 1,
        true,
    ); //get the needed size
    copy_code_block_to_end(left_most_end, code_block_size);
    write_next_pointer(current_next, left_most_end);

    #[cfg(feature = "condition")]
    {
        let (right_block_size, _) = code_block::read_from_right(get_right_most_end(left_most_end));
        //the new code blocks must have the same value
        assert!(code_block::read_from_left(left_most_end) == right_block_size,);
        //the next pointers must be the same
        assert!(
            code_block::read_from_left(left_most_end) < 8
                || *get_left_next(left_most_end, code_block::get_block_size(left_most_end))
                    == *get_right_next(left_most_end, code_block::get_block_size(left_most_end)),
        );
    }
}

/// Writes the next Pointer(s) to the correct position(s). Intended as replacement for the copynext_pointer*-functions.
/// #### next_pointer
/// the offset to be written
/// #### left_code_block
/// the left CodeBlock of the Space whose pointers shall be written
pub unsafe fn write_next_pointer(next_pointer: NextPointerType, left_code_block: *const u8) {
    #[cfg(feature = "condition")]
    {
        assert!(code_block::is_free(left_code_block)); //We shouldn't write next_pointers in used areas
    }
    let code_block_size = code_block::get_block_size(left_code_block);
    let space_size = code_block::read_from_left(left_code_block);
    let left_next = (left_code_block as usize + code_block_size) as *mut NextPointerType;
    let right_next = (left_code_block as usize + space_size + code_block_size
        - size_of::<NextPointerType>()) as *mut NextPointerType;
    *left_next = next_pointer;
    if space_size >= 8 {
        *right_next = next_pointer;
    }
    #[cfg(feature = "condition")]
    {
        assert!(code_block::is_free(left_code_block));
        assert!(
            code_block::read_from_left(left_code_block)
                == code_block::read_from_left(
                    (left_code_block as usize + space_size + code_block_size) as *const u8
                ),
        );
        assert!(space_size < 8 || *left_next == *right_next);
    }
}

#[inline]
fn get_left_next(left_most_end: *const u8, code_block_size: usize) -> *mut NextPointerType {
    (left_most_end as usize + code_block_size) as *mut NextPointerType
}

#[inline]
fn get_right_next(left_most_end: *const u8, code_block_size: usize) -> *mut NextPointerType {
    ((get_right_most_end(left_most_end) as usize - code_block_size) - size_of::<NextPointerType>()
        + 1) as *mut NextPointerType //uint32_t is 4 byte in contrast to the one byte right_most_end pointer
}
