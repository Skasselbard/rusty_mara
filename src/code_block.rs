use crate::globals::*;
///
/// The code block which encodes the size of the memory block dynamically
///
/// A code block consist of one ore more bytes. The first bit encodes if there belong more bytes to the block
/// or if this one is the last. In the first byte the second bit encodes if the block is free. The rest of the bytes
/// encode the size of the memory block
///
/// To encode and decode the codeBlock, an automaton is used
/// In the first state, it examines if there are more than on byte needed to encode the memory block size.
/// If more than one byte is needed to encode the size, the second state is entered and the first bit of each byte
/// have another meaning
///
/// If the codeBlock size is encoded with one byte, the first bit is a 1
/// If the codeBlock size is encoded with more than one byte, the first bit is a 0 if it is an ending byte and 1 if
/// it is a non ending byte
///
/// Examples:
/// Legend:
///      0 or 1  - bit which is used to encode the codeBlock size
///      f       - free-bit 1=free, 0=used
///      x       - bit which is used to encode the memory block size
///      |       - byte delimiter
///      .       - half byte delimiter
///
///  1. Memory block size < 2⁷ byte
///      CodeBlock: 1fxx.xxxx
///
///  2. 2⁷ < Memory block size < 2¹⁴
///      CodeBlock: 0fxx.xxxx | 0xxx.xxxx
///
///  3. 2¹⁴ < Memory block size
///      CodeBlock: 0fxx.xxxx | 1xxx.xxxx | 0xxx.xxxx
///      CodeBlock: 0fxx.xxxx | 1xxx.xxxx | 1xxx.xxxx | 0xxx.xxxx
///                  .
///                  .
///                  .
////
use crate::AllocationData;
use core::mem::size_of;

const FREE_BIT: u8 = 0b0100_0000;
const SIZE_BIT: u8 = 0b1000_0000;
const FIRST_DATA_MASK: usize = 0b0011_1111;
const CONTINUE_DATA_MASK: usize = 0b0111_1111;

/// Reads the CodeBlock from the left
/// #### first_byte
/// the first byte from the CodeBlock
/// #### return
/// the size of the memory block
pub unsafe fn read_from_left(first_byte: *const u8) -> usize {
    #[cfg(feature = "consistency-checks")]
    {}
    let mut size: usize;
    if *first_byte & SIZE_BIT > 0 {
        //block is single byte
        size = *first_byte as usize & FIRST_DATA_MASK;
        #[cfg(feature = "consistency-checks")]
        {
            debug_assert!(size <= FIRST_DATA_MASK && size >= size_of::<NextPointerType>()); //dynamic blocks are at least 4 bytes big#
            assert!(*first_byte & SIZE_BIT > 0); //first bit must be set
        }
    } else {
        //block is more than one byte
        let mut current_byte = first_byte.offset(1);
        size = *first_byte as usize & FIRST_DATA_MASK;
        size <<= 7;
        while *current_byte & SIZE_BIT > 0 {
            size |= *current_byte as usize & CONTINUE_DATA_MASK; //insert the last 7 bits of the current byte at the end of size
            current_byte = current_byte.offset(1);
            size <<= 7; //shift the old byte 7  bits to the left to make space for the next 7 bits
        }
        size |= *current_byte as usize & CONTINUE_DATA_MASK; //insert the last 7 bits of the current byte at the end of size
        #[cfg(feature = "consistency-checks")]
        {
            assert!(size >= size_of::<NextPointerType>()); //dynamic blocks are at least 4 bytes big
            assert!(*first_byte & SIZE_BIT == 0); //first bit of the first byte must not be set
        }
    }
    size
}
/// Reads the CodeBlock from the right
/// #### first_byte
/// the rightmost byte from the CodeBlock
/// #### return
/// the size of the memory block and the left most byte of the block
pub unsafe fn read_from_right(first_byte: *const u8) -> (usize, *mut u8) {
    #[cfg(feature = "consistency-checks")]
    {}
    let mut out_left_byte = first_byte;
    let mut size: usize;
    if *first_byte & SIZE_BIT > 0 {
        //block is single byte
        size = *first_byte as usize & FIRST_DATA_MASK;
        #[cfg(feature = "consistency-checks")]
        {
            assert!(size <= FIRST_DATA_MASK && size >= size_of::<NextPointerType>()); //dynamic blocks are at least 4 bytes big#
            assert!(*first_byte & SIZE_BIT > 0); //first bit must be set
        }
    } else {
        //block is more than one byte
        let mut current_byte = first_byte.offset(-1);
        size = *first_byte as usize & CONTINUE_DATA_MASK;
        let mut m = 1;
        while *current_byte & SIZE_BIT > 0 {
            let mut tmp = *current_byte as usize & CONTINUE_DATA_MASK; //stuff the 7 bits into a temporary size_t
            tmp <<= 7 * m; //shift them to the appropriate position
            size |= tmp as usize; //merge size and tmp
            current_byte = current_byte.offset(-1);
            m = m + 1;
        }
        let mut tmp = *current_byte as usize & FIRST_DATA_MASK; //stuff the 7 bits into a temporary size_t
        tmp <<= 7 * m; //shift them to the appropriate position
        size |= tmp; //merge size and tmp
        out_left_byte = current_byte;
        #[cfg(feature = "consistency-checks")]
        {
            assert!(size >= size_of::<NextPointerType>()); //dynamic blocks are at least 4 bytes big
            assert!(*out_left_byte & SIZE_BIT == 0); //first bit must not be set
            assert!(out_left_byte < first_byte); //first byte must be befor the last byte
            assert!(*first_byte & SIZE_BIT == 0); //first bit of the last byte must not be set
        }
    }
    (size, out_left_byte as *mut u8)
}

/// Build a CodeBlock for a payload with the given size (from the right side of the
/// left codeBlock to the left side of the right code block). Useful to allocate the
/// memory for a new occupied space.
/// Updates the code block size from ``alloc_data``.
pub unsafe fn generate_code_block_for_payload_size(alloc_data: &mut AllocationData, isfree: bool) {
    let mut code_block_size;
    if alloc_data.space_size() <= FIRST_DATA_MASK {
        code_block_size = 1;
        *alloc_data.data_start() = (alloc_data.space_size() | SIZE_BIT as usize) as u8;
        set_free(alloc_data.data_start(), isfree);
    } else {
        //calculate how many bytes are needed
        let mut t: usize = alloc_data.space_size() >> 6;
        code_block_size = 2;
        while t > CONTINUE_DATA_MASK {
            t >>= 7;
            code_block_size += 1;
        }
        generate_code_block_for_payload_size2(
            alloc_data.data_start(),
            alloc_data.space_size(),
            isfree,
            code_block_size,
        );
    }
    alloc_data.set_code_block_size(code_block_size);
    alloc_data.set_space(alloc_data.data_start().add(alloc_data.code_block_size()));
}

/// Build a CodeBlock for space that is managed internally (from the left side of the left codeBlock to the right side
/// of the right code block). Useful to allocate the memory for a new free space.
/// #### left_start_of_block
/// the beginning of the codeBlock starting from the left (return and this pointer should be the same)
/// #### internallyNeededSize
/// size of the internally occupied space including management information
/// #### return
/// size of code block and an array of bytes, containing the codeBlock representing the size between the both codeBlocks.
pub unsafe fn generate_code_block_for_internal_size(
    left_start_of_block: *mut u8,
    internally_needed_size: usize,
    isfree: bool,
) -> usize {
    #[cfg(feature = "consistency-checks")]
    {
        assert!(internally_needed_size >= size_of::<NextPointerType>()); //trivial.
    }
    let mut code_block_size = 1;
    while get_needed_code_block_size(internally_needed_size - 2 * code_block_size) > code_block_size
    {
        code_block_size = code_block_size + 1;
    }
    generate_code_block_for_payload_size2(
        left_start_of_block,
        internally_needed_size - 2 * code_block_size,
        isfree,
        code_block_size,
    );
    #[cfg(feature = "consistency-checks")]
    {
        assert!(code_block_size == get_block_size(left_start_of_block));
        assert!(match isfree {
            true => *left_start_of_block & FREE_BIT > 0,
            false => *left_start_of_block & FREE_BIT == 0,
        });
        assert!(
            read_from_left(left_start_of_block) >= internally_needed_size - 2 * code_block_size
        );
    }
    code_block_size
}

/// reads if the given CodeBlock describes a free or used block.
/// #### first_byte
/// the first byte of the codeBlock, from the left
/// #### return
/// 0 if used, !=0 otherwise
#[inline]
pub fn is_free(first_byte: *const u8) -> bool {
    unsafe { (*first_byte & FREE_BIT) == FREE_BIT }
}

/// reads the size of the block in bytes
/// #### first_byte
/// the first byte of the codeBlock, from the left
/// #### return
/// the number of bytes used by this block
pub unsafe fn get_block_size(first_byte: *const u8) -> usize {
    #[cfg(feature = "consistency-checks")]
    {}
    if *first_byte & SIZE_BIT > 0 {
        return 1;
    }
    let mut current_byte = first_byte.offset(1);
    let mut size: usize = 2;
    while *current_byte & SIZE_BIT > 0 {
        current_byte = current_byte.offset(1);
        size = size + 1;
    }
    #[cfg(feature = "consistency-checks")]
    {
        assert!(size > 1);
    }
    size
}
/// set if the CodeBlock represents a free block or a used one
/// #### first_byte
/// the first byte of the codeBlock, from the left
/// #### free
/// true to mark it as free, false otherwise
#[inline]
pub unsafe fn set_free(first_byte: *mut u8, free: bool) {
    if free {
        *first_byte |= FREE_BIT
    } else {
        *first_byte &= 191
    }
    #[cfg(feature = "consistency-checks")]
    {
        assert!(is_free(first_byte) == free);
    }
}
/// calculates how many bytes a codeBlock would need to encode a given block size
/// #### sizeToEncode
/// the block size to encode
/// #### return
/// the size of the resulting code block
#[inline]
pub fn get_needed_code_block_size(mut size_to_encode: usize) -> usize {
    #[cfg(feature = "consistency-checks")]
    {
        assert!(size_to_encode > 0); //trivial.
    }
    if size_to_encode < FREE_BIT as usize {
        return 1;
    }
    let mut size: usize = 1;
    size_to_encode >>= 6;
    while size_to_encode != 0 {
        size += 1;
        size_to_encode >>= 7;
    }
    #[cfg(feature = "consistency-checks")]
    {
        assert!(size > 1); //trivial.
    }
    return size;
}

/// Build a CodeBlock for a payload with the given size and a given size of the code block  
/// #### left_start_of_block
/// the beginning of the codeBlock starting from the left (return and this pointer should be the same)  
/// #### memory_block_size
/// size of the memory block which should be represented by the CodeBlock  
/// #### isfree
///  wether the codeBlock encode a free or used space  
/// #### code_block_size
/// size of the CodeBlock in Bytes  
/// #### return
/// an array of bytes, containing the codeBlock representing the memory block size.  
unsafe fn generate_code_block_for_payload_size2(
    left_start_of_block: *mut u8,
    mut memory_block_size: usize,
    isfree: bool,
    code_block_size: usize,
) {
    #[cfg(feature = "consistency-checks")]
    {
        assert!(memory_block_size >= size_of::<NextPointerType>());
        assert!(code_block_size > 0);
    }
    if code_block_size == 1 {
        *left_start_of_block = (memory_block_size | SIZE_BIT as usize) as u8;
        set_free(left_start_of_block, isfree);
        #[cfg(feature = "consistency-checks")]
        {
            assert!(*left_start_of_block & SIZE_BIT > 0);
            assert!(is_free(left_start_of_block) == isfree);
            assert!(read_from_left(left_start_of_block) == memory_block_size);
        }
    }

    //write the bytes right to left
    let mut current = left_start_of_block.offset((code_block_size - 1) as isize);
    let mut last = true;
    for _ in 0..code_block_size {
        if last {
            //current is the rightmost byte
            *current = (memory_block_size & CONTINUE_DATA_MASK) as u8;
            memory_block_size >>= 7;
            last = false;
            current = current.offset(-1);
        } else if current == left_start_of_block {
            //current is the leftmost byte
            *current = (memory_block_size & FIRST_DATA_MASK) as u8;
            set_free(left_start_of_block, isfree);
            #[cfg(feature = "consistency-checks")]
            {
                assert!(*left_start_of_block & SIZE_BIT == 0);
                assert!(is_free(left_start_of_block) == isfree);
            }
            // this was the last byte
            break;
        } else {
            *current = ((memory_block_size & CONTINUE_DATA_MASK) | SIZE_BIT as usize) as u8;
            memory_block_size >>= 7;
            current = current.offset(-1);
        }
    }
}
