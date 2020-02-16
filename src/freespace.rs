// #![cfg_attr(feature = "bit", feature(bit))]

use crate::codeblock;
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
pub unsafe fn getNext(left_most_end: *const u8, startOfPage: *const u8) -> *mut u8 {
    let codeBlockSize = codeblock::getBlockSize(left_most_end);
    let leftNext = getLeftNext(left_most_end, codeBlockSize);
    if *leftNext == ERROR_NEXT_POINTER {
        return core::ptr::null_mut();
    }
    (startOfPage.offset(*leftNext as isize)) as *mut u8
}

/// Adapt the next pointer in the data structure. The next pointer is adjacent to the code blocks and stored
/// as a 4 byte integer interpreted as offset from the last byte (to the right)
/// #### next
/// pointer to the next free space. Can be null. If null the offset is set to 0 which will be interpreted
/// as if there is no successor.
/// #### startOfPage
/// the start of the page the space is in. Is needed to calculate the offset that is actually
/// saved in the space
pub unsafe fn setNext(left_most_end: *const u8, next: *const u8, startOfPage: *const u8) {
    #[cfg(feature = "condition")]
    {
        assert!(next.is_null() || next as *const u8 >= startOfPage);
        assert!((next as usize - startOfPage as usize) < 4294967295); // offset is less than uint 32
    }
    let left_most_end = left_most_end;
    let codeBlockSize = codeblock::getBlockSize(left_most_end);
    let leftNext = getLeftNext(left_most_end, codeBlockSize);
    let rightNext = getRightNext(left_most_end, codeBlockSize);
    if next == core::ptr::null() {
        *leftNext = ERROR_NEXT_POINTER;
        *rightNext = ERROR_NEXT_POINTER;
        #[cfg(feature = "condition")]
        {}
        return;
    }
    let offset = (next as usize - startOfPage as usize) as NextPointerType;
    *leftNext = offset;
    if codeblock::readFromLeft(left_most_end) >= 8 {
        //overlapping pointers if the size is too little
        *rightNext = offset;
    }
    #[cfg(feature = "condition")]
    {
        assert!(getNext(left_most_end, startOfPage) as *const u8 >= startOfPage);
        assert!(*leftNext != ERROR_NEXT_POINTER);
        assert!(*rightNext != ERROR_NEXT_POINTER);
    }
}

/// Adjust the size of the free space.<br/>
/// The right most end will be the same as before. The left most end will be the given byte. The next pointer
/// from the free space will be copied from the right to the left
/// #### firstByte
/// the new first byte of the space
/// #### return
/// a pointer to the left most byte of the free space (should be the same as the input)
pub unsafe fn pushBeginningRight(left_most_end: *const u8, firstByte: *mut u8) -> *mut u8 {
    #[cfg(feature = "condition")]
    {
        assert!(
            firstByte > left_most_end as *mut u8
                && firstByte < getRightMostEnd(left_most_end) as *mut u8
        );
        assert!(firstByte < getRightMostEnd(left_most_end) as *mut u8); //Never cross the pointers!
    }
    let codeBlockSize = codeblock::getBlockSize(left_most_end);
    let rightMostEnd = getRightMostEnd(left_most_end);
    let nextPointer = *getRightNext(left_most_end, codeBlockSize);
    let (_, block) = codeblock::getCodeBlockForInternalSize(
        firstByte,
        (rightMostEnd as usize - firstByte as usize) + 1,
        true,
    );
    if firstByte == block {
        copyCodeBlockToEnd(firstByte, codeBlockSize);
        writeNextPointer(nextPointer, firstByte);
        #[cfg(feature = "condition")]
        {
            let newFreeSpace: *const u8 = firstByte;
            let (right_block_size, _) = codeblock::readFromRight(rightMostEnd);
            assert!(codeblock::readFromLeft(firstByte) == right_block_size,);
            assert!(
                codeblock::readFromLeft(firstByte) < 8
                    || *(getLeftNext(newFreeSpace, codeblock::getBlockSize(firstByte)))
                        == *(getRightNext(newFreeSpace, codeblock::getBlockSize(firstByte))),
            );
        }
        return firstByte;
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
/// #### lastByte
/// the new last byte of the space
pub unsafe fn pushEndLeft(left_most_end: *const u8, lastByte: *const u8) {
    #[cfg(feature = "condition")]
    {
        assert!(lastByte > left_most_end); //the new last byte must be in the old freespace
        assert!(lastByte <= getRightMostEnd(left_most_end)); //see above
    }
    let codeBlockSize = codeblock::getBlockSize(left_most_end);
    let currentNext = *getLeftNext(left_most_end, codeBlockSize); //Needed incase the new CodeBlocks are smaller
    codeblock::getCodeBlockForInternalSize(
        left_most_end as *mut u8,
        (lastByte as usize - left_most_end as usize) + 1,
        true,
    ); //get the needed size
    copyCodeBlockToEnd(left_most_end, codeBlockSize);
    writeNextPointer(currentNext, left_most_end);

    #[cfg(feature = "condition")]
    {
        let (right_block_size, _) = codeblock::readFromRight(getRightMostEnd(left_most_end));
        //the new code blocks must have the same value
        assert!(codeblock::readFromLeft(left_most_end) == right_block_size,);
        //the next pointers must be the same
        assert!(
            codeblock::readFromLeft(left_most_end) < 8
                || *getLeftNext(left_most_end, codeblock::getBlockSize(left_most_end))
                    == *getRightNext(left_most_end, codeblock::getBlockSize(left_most_end)),
        );
    }
}

/// Writes the next Pointer(s) to the correct position(s). Intended as replacement for the copyNextPointer*-functions.
/// #### nextPointer
/// the offset to be written
/// #### leftCodeBlock
/// the left CodeBlock of the Space whose pointers shall be written
pub unsafe fn writeNextPointer(nextPointer: NextPointerType, leftCodeBlock: *const u8) {
    #[cfg(feature = "condition")]
    {
        assert!(codeblock::isFree(leftCodeBlock)); //We shouldn't write NextPointers in used areas
    }
    let codeBlockSize = codeblock::getBlockSize(leftCodeBlock);
    let spaceSize = codeblock::readFromLeft(leftCodeBlock);
    let leftNext = (leftCodeBlock as usize + codeBlockSize) as *mut NextPointerType;
    let rightNext = (leftCodeBlock as usize + spaceSize + codeBlockSize
        - size_of::<NextPointerType>()) as *mut NextPointerType;
    *leftNext = nextPointer;
    if spaceSize >= 8 {
        *rightNext = nextPointer;
    }
    #[cfg(feature = "condition")]
    {
        assert!(codeblock::isFree(leftCodeBlock));
        assert!(
            codeblock::readFromLeft(leftCodeBlock)
                == codeblock::readFromLeft(
                    (leftCodeBlock as usize + spaceSize + codeBlockSize) as *const u8
                ),
        );
        assert!(spaceSize < 8 || *leftNext == *rightNext);
    }
}

#[inline]
unsafe fn copyNextPointerFromEndToFront(
    front: *mut NextPointerType,
    end: *const NextPointerType,
) -> bool {
    *front = *end;
    return true;
}
#[inline]
unsafe fn copyNextPointerFromFrontToEnd(
    front: *const NextPointerType,
    end: *mut NextPointerType,
) -> bool {
    *end = *front;
    return true;
}
#[inline]
fn getLeftNext(left_most_end: *const u8, codeBlockSize: usize) -> *mut NextPointerType {
    (left_most_end as usize + codeBlockSize) as *mut NextPointerType
}

#[inline]
fn getRightNext(left_most_end: *const u8, codeBlockSize: usize) -> *mut NextPointerType {
    ((getRightMostEnd(left_most_end) as usize - codeBlockSize) - size_of::<NextPointerType>() + 1)
        as *mut NextPointerType //uint32_t is 4 byte in contrast to the one byte rightMostEnd pointer
}
