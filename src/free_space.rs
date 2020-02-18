// #![cfg_attr(feature = "bit", feature(bit))]

use crate::code_block;
use crate::space::*;
use crate::{AllocationData, MaraError};
use core::mem::size_of;
use core::ptr::*;

#[cfg(feature = "bit32")]
pub type NextPointerType = u32;
#[cfg(feature = "bit16")]
pub type NextPointerType = u16;
#[cfg(feature = "bit8")]
pub type NextPointerType = u8;

const ERROR_NEXT_POINTER: NextPointerType = NextPointerType::max_value(); // just ones

/// #### return
/// a pointer to the next FreeSpace in the bucket. The left next pointer will be taken as reference.
/// Next pointer are an offset of the page start
/// nullptr if the offset == 0
#[inline]
pub unsafe fn get_next(alloc_data: &AllocationData) -> Result<*mut u8, MaraError> {
    let left_next = get_left_next(alloc_data)?;
    if *left_next == ERROR_NEXT_POINTER {
        return Ok(null_mut());
    }
    Ok((alloc_data.start_of_page()?.add(*left_next as usize)) as *mut u8)
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
pub unsafe fn set_next(alloc_data: &mut AllocationData, next: *const u8) -> Result<(), MaraError> {
    #[cfg(feature = "consistency-checks")]
    {
        assert!(next.is_null() || next as *const u8 >= alloc_data.start_of_page()?);
        assert!(
            (next as usize).saturating_sub(alloc_data.start_of_page()? as usize)
                < u32::max_value() as usize
        );
        // offset is less than uint 32
    }
    let left_next = get_left_next(alloc_data)?;
    let right_next = get_right_next(alloc_data)?;
    if next == core::ptr::null() {
        *left_next = ERROR_NEXT_POINTER;
        *right_next = ERROR_NEXT_POINTER;
        #[cfg(feature = "consistency-checks")]
        {}
        return Ok(());
    }
    let offset = (next.sub(alloc_data.start_of_page()? as usize)) as NextPointerType;
    *left_next = offset;
    if code_block::read_from_left(alloc_data.data_start()?) >= size_of::<NextPointerType>() {
        //overlapping pointers if the size is too little
        *right_next = offset;
    }
    #[cfg(feature = "consistency-checks")]
    {
        assert!(get_next(alloc_data)? as *const u8 >= alloc_data.start_of_page()?);
        assert!(*left_next != ERROR_NEXT_POINTER);
        assert!(*right_next != ERROR_NEXT_POINTER);
    }
    Ok(())
}

/// Adjust the size of the free space.<br/>
/// The right most end will be the same as before. The left most end will be the given byte. The next pointer
/// from the free space will be copied from the right to the left
/// #### first_byte
/// the new first byte of the space
/// #### return
/// a pointer to the left most byte of the free space (should be the same as the input)
pub unsafe fn push_beginning_right(
    alloc_data: &mut AllocationData,
    to: *mut u8,
) -> Result<(), MaraError> {
    #[cfg(feature = "consistency-checks")]
    {
        assert!(to > alloc_data.data_start()? && to < alloc_data.data_end()?);
        assert!(to < alloc_data.data_end()?); //Never cross the pointers!
    }
    alloc_data.set_code_block_size(code_block::get_block_size(alloc_data.data_start()?));
    alloc_data.set_data_start(to);
    #[allow(dead_code)]
    get_right_next(alloc_data)?;
    let (_, block) = code_block::get_code_block_for_internal_size(
        alloc_data.data_start()?,
        alloc_data.data_size()? + 1,
        true,
    );
    alloc_data.set_data_start(block);
    alloc_data.set_code_block_size(code_block::get_block_size(block));
    copy_code_block_to_end(alloc_data)?;
    write_next_pointer(alloc_data, *get_right_next(alloc_data)?)?;
    #[cfg(feature = "consistency-checks")]
    {
        let (right_block_size, _) = code_block::read_from_right(alloc_data.data_end()?);
        assert!(code_block::read_from_left(alloc_data.data_start()?) == right_block_size,);
        get_left_next(alloc_data)?;
        let left_next = alloc_data.next_pointer()?;
        get_right_next(alloc_data)?;
        let right_next = alloc_data.next_pointer()?;
        assert!(
            code_block::read_from_left(alloc_data.data_start()?) < size_of::<NextPointerType>()
                || *(left_next) == *(right_next)
        );
    }
    Ok(())
}
/// Adjust the size of the free space.<br/>
/// The left most end will be the same as before. The right most end will be the given byte. The next pointer
/// from the free space will be copied from the left to the right
/// #### to
/// the new last byte of the space
pub unsafe fn push_end_left(alloc_data: &mut AllocationData, to: *mut u8) -> Result<(), MaraError> {
    #[cfg(feature = "consistency-checks")]
    {
        assert!(to > alloc_data.data_start()?); //the new last byte must be in the old freespace
        assert!(to <= alloc_data.data_end()?); //see above
    }
    let current_next = *get_left_next(alloc_data)?; //Needed in case the new CodeBlocks are smaller
    alloc_data.set_data_end(to);
    code_block::get_code_block_for_internal_size(
        alloc_data.data_start()?,
        alloc_data.data_size()? + 1,
        true,
    ); //get the needed size
    alloc_data.set_code_block_size(code_block::get_block_size(alloc_data.data_start()?));
    copy_code_block_to_end(alloc_data)?;
    write_next_pointer(alloc_data, current_next)?;

    #[cfg(feature = "consistency-checks")]
    {
        let (right_block_size, _) = code_block::read_from_right(alloc_data.data_end()?);
        //the new code blocks must have the same value
        assert!(code_block::read_from_left(alloc_data.data_start()?) == right_block_size,);
        //the next pointers must be the same
        assert!(
            code_block::read_from_left(alloc_data.data_start()?) < size_of::<NextPointerType>()
                || *get_left_next(alloc_data)? == *get_right_next(alloc_data)?,
        );
    }
    Ok(())
}

/// Writes the next Pointer(s) to the correct position(s).
/// #### next_pointer
/// the offset to be written
pub unsafe fn write_next_pointer(
    alloc_data: &mut AllocationData,
    next_pointer: NextPointerType,
) -> Result<(), MaraError> {
    #[cfg(feature = "consistency-checks")]
    {
        assert!(code_block::is_free(alloc_data.data_start()?)); //We shouldn't write next_pointers in used areas
    }
    alloc_data.set_space_size(code_block::read_from_left(alloc_data.data_start()?));
    let left_next = get_left_next(alloc_data)?;
    let right_next = get_right_next(alloc_data)?;
    *left_next = next_pointer;
    if alloc_data.space_size()? >= size_of::<NextPointerType>() {
        *right_next = next_pointer;
    }
    #[cfg(feature = "consistency-checks")]
    {
        assert!(code_block::is_free(alloc_data.data_start()?));
        assert!(
            code_block::read_from_left(alloc_data.data_start()?)
                == code_block::read_from_left(alloc_data.code_block_right()?),
        );
        assert!(
            alloc_data.space_size()? < size_of::<NextPointerType>() || *left_next == *right_next
        );
    }
    Ok(())
}

/// sets the next pointer cache in the alloc data to the value of the left next pointer
/// in the data array
#[inline]
pub fn get_left_next(alloc_data: &AllocationData) -> Result<*mut NextPointerType, MaraError> {
    Ok(unsafe {
        alloc_data.data_start()?.add(alloc_data.code_block_size()?) as *mut NextPointerType
    })
}

/// sets the next pointer cache in the alloc data to the value of the right next pointer
/// in the data array
#[inline]
pub fn get_right_next(alloc_data: &AllocationData) -> Result<*mut NextPointerType, MaraError> {
    Ok(unsafe {
        alloc_data
            .data_end()?
            .sub(alloc_data.code_block_size()?)
            .sub(size_of::<NextPointerType>())
            .add(1) as *mut NextPointerType
    })
}
