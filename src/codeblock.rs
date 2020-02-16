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

/// Reads the CodeBlock from the left
/// #### firstByte
/// the first byte from the CodeBlock
/// #### return
/// the size of the memory block
pub unsafe fn readFromLeft(firstByte: *const u8) -> usize {
    #[cfg(feature = "condition")]
    {}
    let mut size: usize;
    if *firstByte & 128 > 0 {
        //block is single byte
        size = (*firstByte & 63) as usize;
        #[cfg(feature = "condition")]
        {
            debug_assert!(size <= 63 && size >= 4); //dynamic blocks are at least 4 bytes big#
            assert!(*firstByte & 128 > 0); //first bit must be set
        }
    } else {
        //block is more than one byte
        let mut currentByte = firstByte.offset(1);
        size = (*firstByte & 63) as usize;
        size <<= 7;
        while *currentByte & 128 > 0 {
            size |= (*currentByte & 127) as usize; //insert the last 7 bits of the current byte at the end of size
            currentByte = currentByte.offset(1);
            size <<= 7; //shift the old byte 7  bits to the left to make space for the next 7 bits
        }
        size |= (*currentByte & 127) as usize; //insert the last 7 bits of the current byte at the end of size
    }
    #[cfg(feature = "condition")]
    {
        assert!(size >= 4); //dynamic blocks are at least 4 bytes big
        assert!(*firstByte & 128 == 0); //first bit of the first byte must not be set
    }
    size
}
/// Reads the CodeBlock from the right
/// #### firstByte
/// the rightmost byte from the CodeBlock
/// #### return
/// the size of the memory block and the left most byte of the block
pub unsafe fn readFromRight(firstByte: *const u8) -> (usize, *const u8) {
    #[cfg(feature = "condition")]
    {}
    let mut outLeftByte = firstByte;
    let mut size: usize;
    if *firstByte & 128 > 0 {
        //block is single byte
        size = (*firstByte & 63) as usize;
        #[cfg(feature = "condition")]
        {
            assert!(size <= 63 && size >= 4); //dynamic blocks are at least 4 bytes big#
            assert!(*firstByte & 128 > 0); //first bit must be set
        }
    } else {
        //block is more than one byte
        let mut currentByte = firstByte.offset(-1);
        size = (*firstByte & 127) as usize;
        let mut m = 1;
        while *currentByte & 128 > 0 {
            let mut tmp = *currentByte & 127; //stuff the 7 bits into a temporary size_t
            tmp <<= 7 * m; //shift them to the appropriate position
            size |= tmp as usize; //merge size and tmp
            currentByte = currentByte.offset(-1);
            m = m + 1;
        }
        let mut tmp = (*currentByte & 63) as usize; //stuff the 7 bits into a temporary size_t
        tmp <<= 7 * m; //shift them to the appropriate position
        size |= tmp; //merge size and tmp
        outLeftByte = currentByte;
        #[cfg(feature = "condition")]
        {
            assert!(size >= 4); //dynamic blocks are at least 4 bytes big
            assert!(*outLeftByte & 128 == 0); //first bit must not be set
            assert!(outLeftByte < firstByte); //first byte must be befor the last byte
            assert!(*firstByte & 128 == 0); //first bit of the last byte must not be set
        }
    }
    (size, outLeftByte)
}

/// Build a CodeBlock for a payload with the given size (from the right side of the left codeBlock to the left side
/// of the right code block). Useful to allocate the memory for a new occupied space.
/// #### leftStartOfBlock
/// the beginning of the codeBlock starting from the left (return and this pointer should be the same)
/// #### memoryBlockSize
/// size of the memory block which should be represented by the CodeBlock
/// #### returnArraySize
/// size of the array returned by this function
/// #### return
/// an array of bytes, containing the codeBlock representing the memory block size.
/// The size of the array is stored in the second to last parameter. It should correspond to the leftStartOfBlock parameter
pub unsafe fn getCodeBlockForPayloadSize(
    leftStartOfBlock: *mut u8,
    memoryBlockSize: usize,
    returnArraySize: *mut usize,
    isFree: bool,
) -> *const u8 {
    if memoryBlockSize <= 63 {
        *returnArraySize = 1;
        *leftStartOfBlock = (memoryBlockSize | 128) as u8;
        setFree(leftStartOfBlock, isFree);
        return leftStartOfBlock;
    }
    //calculate how many bytes are needed
    let mut t: usize = memoryBlockSize >> 6;
    *returnArraySize = 2;
    while t > 127 {
        t >>= 7;
        *returnArraySize = *returnArraySize + 1;
    }
    getCodeBlockForPayloadSize2(leftStartOfBlock, memoryBlockSize, isFree, *returnArraySize)
}

/// Build a CodeBlock for space that is managed internally (from the left side of the left codeBlock to the right side
/// of the right code block). Useful to allocate the memory for a new free space.
/// #### leftStartOfBlock
/// the beginning of the codeBlock starting from the left (return and this pointer should be the same)
/// #### internallyNeededSize
/// size of the internally occupied space including management information
/// #### return
/// size of the array and an array of bytes, containing the codeBlock representing the size between the both codeBlocks.
pub unsafe fn getCodeBlockForInternalSize(
    leftStartOfBlock: *mut u8,
    internallyNeededSize: usize,
    isFree: bool,
) -> (usize, *mut u8) {
    #[cfg(feature = "condition")]
    {
        assert!(internallyNeededSize >= 4); //trivial.
    }
    let mut returnArraySize = 1;
    while getNeededCodeBlockSize(internallyNeededSize - 2 * returnArraySize) > returnArraySize {
        returnArraySize = returnArraySize + 1;
    }
    let returnByte = getCodeBlockForPayloadSize2(
        leftStartOfBlock,
        internallyNeededSize - 2 * returnArraySize,
        isFree,
        returnArraySize,
    );
    #[cfg(feature = "condition")]
    {
        assert!(returnArraySize == getBlockSize(leftStartOfBlock));
        assert!(match isFree {
            true => *leftStartOfBlock & 64 > 0,
            false => *leftStartOfBlock & 64 == 0,
        });
        assert!(readFromLeft(leftStartOfBlock) >= internallyNeededSize - 2 * returnArraySize);
    }
    return (returnArraySize, returnByte);
}

/// reads if the given CodeBlock describes a free or used block.
/// #### firstByte
/// the first byte of the codeBlock, from the left
/// #### return
/// 0 if used, !=0 otherwise
#[inline]
pub fn isFree(firstByte: *const u8) -> bool {
    unsafe { *firstByte & 64 == 1 }
}

/// reads the size of the block in bytes
/// #### firstByte
/// the first byte of the codeBlock, from the left
/// #### return
/// the number of bytes used by this block
pub unsafe fn getBlockSize(firstByte: *const u8) -> usize {
    #[cfg(feature = "condition")]
    {}
    if *firstByte & 128 > 0 {
        return 1;
    }
    let mut currentByte = firstByte.offset(1);
    let mut size: usize = 2;
    while *currentByte & 128 > 0 {
        currentByte = currentByte.offset(1);
        size = size + 1;
    }
    #[cfg(feature = "condition")]
    {
        assert!(size > 1);
    }
    size
}
/// set if the CodeBlock represents a free block or a used one
/// #### firstByte
/// the first byte of the codeBlock, from the left
/// #### free
/// 0 to mark it as free, != 0 otherwise
#[inline]
pub unsafe fn setFree(firstByte: *mut u8, free: bool) {
    if free {
        *firstByte |= 64
    } else {
        *firstByte &= 191
    }
    #[cfg(feature = "condition")]
    {
        assert!(isFree(firstByte) == free);
    }
}
/// calculates how many bytes a codeBlock would need to encode a given block size
/// #### sizeToEncode
/// the block size to encode
/// #### return
/// the size of the resulting code block
#[inline]
pub fn getNeededCodeBlockSize(mut sizeToEncode: usize) -> usize {
    #[cfg(feature = "condition")]
    {
        assert!(sizeToEncode > 0); //trivial.
    }
    if sizeToEncode < 64 {
        return 1;
    }
    let mut size: usize = 1;
    sizeToEncode >>= 6;
    while sizeToEncode == 1 {
        size = size + 1;
        sizeToEncode >>= 7;
    }
    #[cfg(feature = "condition")]
    {
        assert!(size > 1); //trivial.
    }
    return size;
}

/// Build a CodeBlock for a payload with the given size and a given size of the code block  
/// #### leftStartOfBlock
/// the beginning of the codeBlock starting from the left (return and this pointer should be the same)  
/// #### memoryBlockSize
/// size of the memory block which should be represented by the CodeBlock  
/// #### isFree
///  wether the codeBlock encode a free or used space  
/// #### codeBlockSize
/// size of the CodeBlock in Bytes  
/// #### return
/// an array of bytes, containing the codeBlock representing the memory block size.  
unsafe fn getCodeBlockForPayloadSize2(
    leftStartOfBlock: *mut u8,
    mut memoryBlockSize: usize,
    is_Free: bool,
    codeBlockSize: usize,
) -> *mut u8 {
    #[cfg(feature = "condition")]
    {
        assert!(memoryBlockSize >= 4);
        assert!(codeBlockSize > 0);
    }
    if codeBlockSize == 1 {
        *leftStartOfBlock = (memoryBlockSize | 128) as u8;
        setFree(leftStartOfBlock, is_Free);
        #[cfg(feature = "condition")]
        {
            assert!(*leftStartOfBlock & 128 > 0);
            assert!(isFree(leftStartOfBlock) == is_Free);
            assert!(readFromLeft(leftStartOfBlock) == memoryBlockSize);
        }
        return leftStartOfBlock;
    }

    //write the bytes right to left
    let mut current = leftStartOfBlock.offset((codeBlockSize - 1) as isize);
    let mut last = true;
    for _ in 0..codeBlockSize {
        if last {
            //current is the rightmost byte
            *current = (memoryBlockSize & 127) as u8;
            memoryBlockSize >>= 7;
            last = false;
            current = current.offset(-1);
        } else if current == leftStartOfBlock {
            //current is the leftmost byte
            *current = (memoryBlockSize & 63) as u8;
            setFree(leftStartOfBlock, is_Free);
            #[cfg(feature = "condition")]
            {
                assert!(*leftStartOfBlock & 128 == 0);
                assert!(isFree(leftStartOfBlock) == is_Free);
            }
            return leftStartOfBlock;
        } else {
            *current = ((memoryBlockSize & 127) | 128) as u8;
            memoryBlockSize >>= 7;
            current = current.offset(-1);
        }
    }
    // should not be reached
    #[cfg(feature = "condition")]
    {
        assert!(false);
    }
    return leftStartOfBlock;
}
