use crate::code_block;
use crate::{AllocationData, MaraError};

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

/// Takes a a Space and returns a Space interpreted as Occupied. The code blocks are adapted accordingly.
/// #### newSize
/// the size to new block should have
/// #### return
/// a pointer to the new space with updated codeBlocks
pub unsafe fn to_occupied(alloc_data: &mut AllocationData) -> Result<(), MaraError> {
    code_block::set_free(alloc_data.data_start()?, false);
    let mut code_block_size: usize = 0;

    code_block::get_code_block_for_payload_size(
        alloc_data.data_start()?,
        alloc_data.data_size()?,
        &mut code_block_size,
        false,
    );
    copy_code_block_to_end(alloc_data)?;
    #[cfg(feature = "consistency-checks")]
    {
        assert!(!code_block::is_free(alloc_data.data_start()?));
        let (right_block_size, _) = code_block::read_from_right(alloc_data.data_end()?);
        assert!(code_block::read_from_left(alloc_data.data_start()?) == right_block_size,);
    }
    Ok(())
}

/// Copies a code block from the beginning of space to the end of space
/// #### start_of_block
/// beginning of the block to copy
/// #### size_of_block
/// amount of bytes the block uses
/// #### return
/// true on success
pub unsafe fn copy_code_block_to_end(alloc_data: &mut AllocationData) -> Result<(), MaraError> {
    #[cfg(feature = "consistency-checks")]
    {
        assert!(alloc_data.code_block_size()? > 0);
    }
    let mut current_position: *mut u8 =
        ((alloc_data.data_end()? as usize - alloc_data.code_block_size()?) + 1) as *mut u8;
    for i in 0..alloc_data.code_block_size()? {
        if current_position as usize <= alloc_data.data_end()? as usize {
            *current_position = *(alloc_data.data_start()?.offset(i as isize));
        } else {
            return Err(MaraError::CodeBlockOverflow);
        }
        current_position = current_position.offset(1);
    }
    #[cfg(feature = "consistency-checks")]
    {
        assert!(current_position.offset(-1) == alloc_data.data_end()?);
        let (right_block_size, _) = code_block::read_from_right(alloc_data.data_end()?);
        assert!(code_block::read_from_left(alloc_data.data_start()?) == right_block_size,);
    }
    Ok(())
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
pub fn get_left_neighbor(alloc_data: &AllocationData) -> Result<*const u8, MaraError> {
    let (memory_size, left_byte) =
        unsafe { code_block::read_from_right(alloc_data.data_start()?.offset(-1)) };
    let code_block_size = code_block::get_needed_code_block_size(memory_size);
    Ok(((left_byte as usize - memory_size) - code_block_size) as *const u8)
}
