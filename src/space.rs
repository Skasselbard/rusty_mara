use crate::codeblock;

/// Basic Structure:<br/>
/// ```
/// Standard Free Space:
/// ------------------------------------------------------------------------------------
/// |.CodeBlock.|.nextPointer.|.........Free Space...........|.nextPointer.|.CodeBlock.|
/// |.min 1byte.|....4byte....|.max PAGE_SIZE - 10 byte byte.|....4byte....|.min 1byte.|
/// ------------------------------------------------------------------------------------
///
/// 6byte Free Space:
/// ---------------------------------------
/// |.CodeBlock.|.nextPointer.|.CodeBlock.|
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
pub fn getSize(left_most_end: *const u8) -> usize {
    (getRightMostEnd(left_most_end) as usize - left_most_end as usize) + 1
}
/// @return the start of the space which could be actual data. <br/>
/// WARNING: if this Space is freeSpace, this pointer might point on the next pointer of the of this free space.
/// This place should only be used if the block is occupied or is immediately converted to occupied space.
pub fn getStartOfSpace(left_most_end: *const u8) -> *mut u8 {
    let codeBlockSize = unsafe { codeblock::getBlockSize(left_most_end) };
    (left_most_end as usize + codeBlockSize) as *mut u8
}

/// #### return
/// the rightmost byte of the entire block, including management information
pub fn getRightMostEnd(left_most_end: *const u8) -> *const u8 {
    let memoryBlockSize = unsafe { codeblock::readFromLeft(left_most_end) };
    let codeBlockSize = unsafe { codeblock::getBlockSize(left_most_end) };
    let rightMostEnd = (left_most_end as usize + (2 * codeBlockSize) + memoryBlockSize) - 1;
    #[cfg(feature = "condition")]
    {
        assert!(rightMostEnd > left_most_end as usize); //trivial.
    }
    rightMostEnd as *const u8
}
/// Takes a a Space and returns a Space interpreted as Occupied. The code blocks are adapted accordingly.
/// #### newSize
/// the size to new block should have
/// #### return
/// a pointer to the new space with updated codeBlocks
pub unsafe fn toOccupied(left_most_end: *mut u8, newSize: usize) {
    codeblock::setFree(left_most_end, false);
    let mut codeBlockSize: usize = 0;

    codeblock::getCodeBlockForPayloadSize(left_most_end, newSize, &mut codeBlockSize, false);
    copyCodeBlockToEnd(left_most_end, codeBlockSize);
    #[cfg(feature = "condition")]
    {
        assert!(!codeblock::isFree(left_most_end));
        let (right_block_size, _) = codeblock::readFromRight(getRightMostEnd(left_most_end));
        assert!(codeblock::readFromLeft(left_most_end) == right_block_size,);
    }
}

/// Copies a code block from the beginning of space to the end of space
/// #### startOfBlock
/// beginning of the block to copy
/// #### sizeOfBlock
/// amount of bytes the block uses
/// #### return
/// true on success
pub unsafe fn copyCodeBlockToEnd(left_most_end: *const u8, sizeOfBlock: usize) -> bool {
    #[cfg(feature = "condition")]
    {
        assert!(sizeOfBlock > 0);
    }
    let right_most_end = getRightMostEnd(left_most_end);
    let mut currentPosition: *mut u8 = ((right_most_end as usize - sizeOfBlock) + 1) as *mut u8;
    for i in 0..sizeOfBlock {
        if currentPosition as usize <= right_most_end as usize {
            *currentPosition = *(left_most_end.offset(i as isize));
        } else {
            #[cfg(feature = "condition")]
            {
                assert!(false);
            }
            return false;
        }
        currentPosition = currentPosition.offset(1);
    }
    #[cfg(feature = "condition")]
    {
        assert!(currentPosition.offset(-1) == getRightMostEnd(left_most_end) as *mut u8);
        let (right_block_size, _) = codeblock::readFromRight(getRightMostEnd(left_most_end));
        assert!(codeblock::readFromLeft(left_most_end) == right_block_size,);
    }
    true
}

/// Copies a code block from the end of space to the beginning of space
/// #### startOfBlock
/// beginning of the block to copy
/// #### sizeOfBlock
/// amount of bytes the block uses
/// #### return
/// true on success
pub unsafe fn copyCodeBlockToFront(
    left_most_end: *mut u8,
    startOfBlock: *const u8,
    sizeOfBlock: usize,
) -> bool {
    let mut currentPosition = left_most_end;
    for i in 0..sizeOfBlock {
        *currentPosition.offset(i as isize) = *startOfBlock.offset(i as isize);
        currentPosition = currentPosition.offset(1);
    }
    true
}

/// #### lastByte
/// of the left neighbor
/// #### return
/// pointer to the left neighbor
pub fn getLeftNeighbor(lastByte: *const u8) -> *const u8 {
    let (memorySize, leftByte) = unsafe { codeblock::readFromRight(lastByte) };
    let codeBlockSize = codeblock::getNeededCodeBlockSize(memorySize);
    ((leftByte as usize - memorySize) - codeBlockSize) as *const u8
}
