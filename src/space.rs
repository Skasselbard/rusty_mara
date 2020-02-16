use crate::code_block;

/// Basic Structure:<br/>
/// ```
/// Standard Free Space:
/// ------------------------------------------------------------------------------------
/// |.CodeBlock.|.next_pointer.|.........Free Space...........|.next_pointer.|.CodeBlock.|
/// |.min 1byte.|....4byte....|.max PAGE_SIZE - 10 byte byte.|....4byte....|.min 1byte.|
/// ------------------------------------------------------------------------------------
///
/// 6byte Free Space:
/// ---------------------------------------
/// |.CodeBlock.|.next_pointer.|.CodeBlock.|
/// |.min 1byte.|....4byte....|.min 1byte.|
/// ---------------------------------------
///
/// Occupied space Space:
/// ------------------------------------------------------------------
/// |.CodeBlock.|...................Data.................|.CodeBlock.|
/// |.min 1byte.|6byte to (max PAGE_SIZE - 10 byte) byte |.min 1byte.|
/// ------------------------------------------------------------------
/// ```
/// #### return
/// The size of the entire space block, including management information
pub fn get_size(left_most_end: *const u8) -> usize {
    (get_right_most_end(left_most_end) as usize - left_most_end as usize) + 1
}
/// @return the start of the space which could be actual data. <br/>
/// WARNING: if this Space is free_space, this pointer might point on the next pointer of the of this free space.
/// This place should only be used if the block is occupied or is immediately converted to occupied space.
pub fn get_start_of_space(left_most_end: *const u8) -> *mut u8 {
    let code_block_size = unsafe { code_block::get_block_size(left_most_end) };
    (left_most_end as usize + code_block_size) as *mut u8
}

/// #### return
/// the rightmost byte of the entire block, including management information
pub fn get_right_most_end(left_most_end: *const u8) -> *const u8 {
    let memory_block_size = unsafe { code_block::read_from_left(left_most_end) };
    let code_block_size = unsafe { code_block::get_block_size(left_most_end) };
    let right_most_end = (left_most_end as usize + (2 * code_block_size) + memory_block_size) - 1;
    #[cfg(feature = "condition")]
    {
        assert!(right_most_end > left_most_end as usize); //trivial.
    }
    right_most_end as *const u8
}
/// Takes a a Space and returns a Space interpreted as Occupied. The code blocks are adapted accordingly.
/// #### newSize
/// the size to new block should have
/// #### return
/// a pointer to the new space with updated codeBlocks
pub unsafe fn to_occupied(left_most_end: *mut u8, new_size: usize) {
    code_block::set_free(left_most_end, false);
    let mut code_block_size: usize = 0;

    code_block::get_code_block_for_payload_size(
        left_most_end,
        new_size,
        &mut code_block_size,
        false,
    );
    copy_code_block_to_end(left_most_end, code_block_size);
    #[cfg(feature = "condition")]
    {
        assert!(!code_block::is_free(left_most_end));
        let (right_block_size, _) = code_block::read_from_right(get_right_most_end(left_most_end));
        assert!(code_block::read_from_left(left_most_end) == right_block_size,);
    }
}

/// Copies a code block from the beginning of space to the end of space
/// #### start_of_block
/// beginning of the block to copy
/// #### size_of_block
/// amount of bytes the block uses
/// #### return
/// true on success
pub unsafe fn copy_code_block_to_end(left_most_end: *const u8, size_of_block: usize) -> bool {
    #[cfg(feature = "condition")]
    {
        assert!(size_of_block > 0);
    }
    let right_most_end = get_right_most_end(left_most_end);
    let mut current_position: *mut u8 = ((right_most_end as usize - size_of_block) + 1) as *mut u8;
    for i in 0..size_of_block {
        if current_position as usize <= right_most_end as usize {
            *current_position = *(left_most_end.offset(i as isize));
        } else {
            #[cfg(feature = "condition")]
            {
                assert!(false);
            }
            return false;
        }
        current_position = current_position.offset(1);
    }
    #[cfg(feature = "condition")]
    {
        assert!(current_position.offset(-1) == get_right_most_end(left_most_end) as *mut u8);
        let (right_block_size, _) = code_block::read_from_right(get_right_most_end(left_most_end));
        assert!(code_block::read_from_left(left_most_end) == right_block_size,);
    }
    true
}

/// Copies a code block from the end of space to the beginning of space
/// #### start_of_block
/// beginning of the block to copy
/// #### size_of_block
/// amount of bytes the block uses
/// #### return
/// true on success
pub unsafe fn copy_code_block_to_front(
    left_most_end: *mut u8,
    start_of_block: *const u8,
    size_of_block: usize,
) -> bool {
    let mut current_position = left_most_end;
    for i in 0..size_of_block {
        *current_position.offset(i as isize) = *start_of_block.offset(i as isize);
        current_position = current_position.offset(1);
    }
    true
}

/// #### last_byte
/// of the left neighbor
/// #### return
/// pointer to the left neighbor
#[inline]
pub fn get_left_neighbor(last_byte: *const u8) -> *const u8 {
    let (memory_size, left_byte) = unsafe { code_block::read_from_right(last_byte) };
    let code_block_size = code_block::get_needed_code_block_size(memory_size);
    ((left_byte as usize - memory_size) - code_block_size) as *const u8
}
