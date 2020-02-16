use core::mem::{size_of_val, transmute};

use crate::bucketlist::BucketList;
use crate::codeblock;
use crate::freespace::*;
use crate::globals::*;
use crate::space::*;

pub struct Page {
    /// Pointer to the first byte of the page
    startOfPage: *const u8,
    /// Pointer to the next page
    nextPage: *mut Self,
    /// pointer to the leftmost byte of the static sector <br/>
    /// the rightmost byte is the last byte of the page
    staticEnd: *const u8,
    ///pointer to the rightmost allocated byte of the dynamic sector <br/>
    ///behind this pointer can only be an allocated chunk form the static
    ///sector. space between this pointer and the staticEnd pointer has to be free memory.
    dynamicEnd: *const u8,
    bucketList: BucketList,
}

impl Page {
    pub fn init(&mut self, page_memory: *mut u8, page_size: usize) -> Self {
        unsafe {
            codeblock::setFree(page_memory, true);
            let bucket_list_memory = *transmute::<*mut u8, *mut [*mut u8; blSize]>(page_memory);
            let bucketlistSize = size_of_val(&bucket_list_memory);
            let startOfPage = page_memory.offset(bucketlistSize as isize);
            let mut bucketList = BucketList::new(bucket_list_memory, page_memory);
            bucketList.addToList(Self::generateFirstBucketEntry(
                startOfPage,
                page_memory.offset(page_size as isize),
            ));
            #[cfg(feature = "condition")]
            {
                for i in 0..(blSize - 1) {
                    assert!(bucketList.getFromBucketList(i).is_null());
                }
                let blockSize = codeblock::getBlockSize(bucketList.getFromBucketList(blSize - 1));
                assert!(
                    codeblock::readFromLeft(bucketList.getFromBucketList(blSize - 1))
                        == page_size - 2 * blockSize,
                );
            }
            Self {
                nextPage: core::ptr::null_mut(),
                startOfPage,
                staticEnd: page_memory.offset(page_size as isize),
                dynamicEnd: page_memory,
                bucketList,
            }
        }
    }
    ////returns a new static block
    pub unsafe fn getStaticBlock(&mut self, sizeInByte: usize) -> *mut u8 {
        #[cfg(feature = "condition")]
        {
            assert!(sizeInByte != 0);
            assert!(self.staticEnd > self.dynamicEnd); //static end should come after the dynamic end
        }
        #[cfg(feature = "align_static")]
        {
            sizeInByte = Self::align(sizeInByte);
        }

        if self.staticBlockFitInPage(sizeInByte) {
            let (codeblocksize, _) = codeblock::readFromRight(self.staticEnd.offset(-1));
            let lastFreeSpace = self.staticEnd.offset(-(3 * codeblocksize as isize)) as *mut u8;
            self.bucketList.deleteFromList(lastFreeSpace);
            self.cutRightFromFreeSpace(lastFreeSpace, sizeInByte);
            self.bucketList.addToList(lastFreeSpace); //lastFreeSpace might get too small for its current bucket
            self.staticEnd = self.staticEnd.offset(-(sizeInByte as isize));
            #[cfg(feature = "condition")]
            {
                assert!(self.staticEnd > self.dynamicEnd); //see above
            }
            return self.staticEnd as *mut u8;
        } else {
            #[cfg(feature = "condition")]
            {
                assert!(self.staticEnd > self.dynamicEnd); //see above
                assert!((self.staticEnd as usize - self.dynamicEnd as usize) < 6 + sizeInByte);
                //there actually shouldn't be enough space
            }
            return core::ptr::null_mut();
        }
    }
    ///returns if a requested block size would fit in the page
    ///checks if there is enough space to begin with and if there would be enough space for a freespace(>6 byte) after insertion
    pub fn staticBlockFitInPage(&self, blockSizeInByte: usize) -> bool {
        //no assertions because state isn't altered
        (blockSizeInByte <= (self.staticEnd as usize - self.dynamicEnd as usize - 1)
            && (self.staticEnd as usize - self.dynamicEnd as usize >= 6 + blockSizeInByte))
    }
    /// tries to reserve a dynamic block in this page, and returns it
    /// #### sizeInByte
    /// the size of the space requested
    /// #### return
    /// a pointer to the space, or nullptr if no space was found
    pub fn getDynamicBlock(&mut self, sizeInByte: usize) -> *mut u8 {
        #[cfg(feature = "condition")]
        {
            assert!(sizeInByte > 0);
            assert!(self.staticEnd > self.dynamicEnd);
        }
        #[cfg(feature = "align_dynamic")]
        {
            sizeInByte = align(sizeInByte);
        }
        let freeSpace = unsafe { self.bucketList.getFreeSpace(sizeInByte) };
        let returnBlock = freeSpace;
        if freeSpace.is_null() {
            #[cfg(feature = "condition")]
            {
                assert!(self.staticEnd > self.dynamicEnd);
            }
            return core::ptr::null_mut();
        } else {
            unsafe { self.bucketList.deleteFromList(freeSpace) };
            let remainingSpace = self.cutLeftFromFreeSpace(
                freeSpace,
                sizeInByte + (2 * codeblock::getNeededCodeBlockSize(sizeInByte)),
            );
            if !remainingSpace.is_null() {
                unsafe { self.bucketList.addToList(remainingSpace) };
                unsafe { toOccupied(returnBlock, sizeInByte) };
            } else {
                //Edge Case: If the remaining space is too small to be used again, simply return a larger block
                unsafe { codeblock::setFree(returnBlock, false) };
                unsafe { copyCodeBlockToEnd(returnBlock, codeblock::getBlockSize(returnBlock)) };
            }

            if getRightMostEnd(returnBlock) > self.dynamicEnd {
                self.dynamicEnd = getRightMostEnd(returnBlock);
            }
        }
        #[cfg(feature = "condition")]
        {
            assert!(!returnBlock.is_null());
            assert!(self.dynamicEnd < self.staticEnd);
            assert!(self.dynamicEnd > self.startOfPage);
            assert!(returnBlock >= self.startOfPage as *mut u8);
            assert!(!codeblock::isFree(returnBlock));
        }
        returnBlock
    }
    /// #### return the next page in the ring storage
    pub fn getNextPage(&self) -> *mut Self {
        self.nextPage
    }
    /// sets the next page
    /// #### nextPage
    /// the next page
    pub fn setNextPage(&mut self, nextPage: *mut Self) {
        if nextPage != core::ptr::null_mut() {}
        self.nextPage = nextPage;
    }
    /// #### firstByte
    /// a pointer to the block of interest
    /// #### return
    /// true if the pointer is in between the start of page and the left most byte of the static sector.
    /// false otherwise. Blocks in the static sector CANNOT be detected with this function.
    pub fn blockIsInSpace(&self, firstByte: *const u8) -> bool {
        self.startOfPage <= firstByte && firstByte < self.staticEnd
    }
    /// deletes a reserved block
    /// #### firstByte
    /// the first byte of the block
    /// #### return
    /// true if successful, false otherwise
    pub fn deleteBlock(&mut self, firstByte: *const u8) -> bool {
        #[cfg(feature = "condition")]
        {
            assert!(self.staticEnd > self.dynamicEnd);
        }
        let (memoryBlockSize, codeBlockStart) =
            unsafe { codeblock::readFromRight(firstByte.offset(-1)) };
        let codeBlockStart = codeBlockStart as *mut u8;
        let codeBlockSize = unsafe { codeblock::getBlockSize(codeBlockStart) };
        #[cfg(feature = "statistic")]
        {
            Statistic::freeDynamic(memoryBlockSize, firstByte);
        }
        if (codeBlockStart as usize + (2 * codeBlockSize) + memoryBlockSize)
            > self.staticEnd as usize
        {
            panic!("code block reaches into static space")
        }
        let mut leftNeighbor = core::ptr::null_mut();
        let mut rightNeighbor =
            (codeBlockStart as usize + (2 * codeBlockSize) + memoryBlockSize) as *mut u8;
        if rightNeighbor as usize > self.staticEnd as usize {
            panic!("dynamic memory links into static space")
        }
        if self.startOfPage < codeBlockStart {
            leftNeighbor = unsafe { getLeftNeighbor(codeBlockStart.offset(-1)) as *mut u8 };
        }
        if !leftNeighbor.is_null() && !codeblock::isFree(leftNeighbor) {
            leftNeighbor = core::ptr::null_mut();
        }
        if !rightNeighbor.is_null()
            && (rightNeighbor as usize >= self.staticEnd as usize
                || !codeblock::isFree(rightNeighbor))
        {
            rightNeighbor = core::ptr::null_mut();
        }
        unsafe { self.mergeFreeSpace(leftNeighbor, codeBlockStart, rightNeighbor) };
        #[cfg(feature = "condition")]
        {
            unsafe {
                assert!(
                    (leftNeighbor.is_null()
                        && self.bucketList.isInList(codeBlockStart).0
                        && codeblock::isFree(codeBlockStart))
                        || (self.bucketList.isInList(leftNeighbor).0
                            && codeblock::isFree(leftNeighbor)),
                )
            };
            assert!(self.staticEnd > self.dynamicEnd);
        }
        return true;
    }
    /// #### return
    /// a pointer to the first byte in the page
    pub fn getStartOfPage(&self) -> *const u8 {
        self.startOfPage
    }
    /// #### return
    /// a pointer to the first byte in the static area
    pub fn getStaticEnd(&self) -> *const u8 {
        self.staticEnd
    }
    /// #### return
    /// the dynamic end
    pub fn getDynamicEnd(&self) -> *const u8 {
        self.dynamicEnd
    }
    /// #### return
    /// the bucket list
    pub fn getBucketList(&self) -> &BucketList {
        &self.bucketList
    }
    /// TODO: describe alignment
    /// align the requested block size
    /// #### requestedSizeInByte
    /// the needed size for the requested block
    /// #### return
    /// the aligned size of the requested block
    fn align(requestedSizeInByte: usize) -> usize {
        unimplemented!()
    }
    /// Merges up to three blocks into one Block of free Space.
    /// Only free blocks are merged.
    /// The bucketList will be updated accordingly<br/>
    /// WARNING: the blocks have to be adjacent to each other. Merging distant blocks will cause undefined behavior.
    /// Probably causing the world as we know it, to cease to exist!
    /// #### leftBlock
    /// leftBlock to be merged. Ignored if null
    /// #### middleBlock
    /// middle Block to be merged
    /// #### rightBlock
    /// right Block to be merged. Ignored if null
    /// #### return
    /// the new block of free space
    unsafe fn mergeFreeSpace(
        &mut self,
        leftBlock: *mut u8,
        middleBlock: *mut u8,
        rightBlock: *mut u8,
    ) -> *const u8 {
        #[cfg(feature = "condition")]
        {
            assert!(!codeblock::isFree(middleBlock));
            assert!(rightBlock.is_null() || self.bucketList.isInList(rightBlock).0);
            assert!(leftBlock.is_null() || self.bucketList.isInList(leftBlock).0);
        }
        if leftBlock.is_null() {
            if !rightBlock.is_null() {
                self.bucketList.deleteFromList(rightBlock);
                self.mergeWithRight(middleBlock, rightBlock);
            }
            codeblock::setFree(middleBlock, true);
            copyCodeBlockToEnd(middleBlock, codeblock::getBlockSize(middleBlock));
            self.bucketList.addToList(middleBlock);
            #[cfg(feature = "condition")]
            {
                assert!(codeblock::isFree(middleBlock));
                assert!(self.bucketList.isInList(middleBlock).0);
            }
            return middleBlock;
        } else {
            if !rightBlock.is_null() {
                self.bucketList.deleteFromList(rightBlock);
                self.mergeWithRight(middleBlock, rightBlock);
            }
            self.bucketList.deleteFromList(leftBlock);

            self.mergeWithLeft(leftBlock, middleBlock);
            codeblock::setFree(leftBlock, true);
            copyCodeBlockToEnd(leftBlock, codeblock::getBlockSize(leftBlock));
            self.bucketList.addToList(leftBlock);
            #[cfg(feature = "condition")]
            {
                assert!(codeblock::isFree(leftBlock));
                assert!(self.bucketList.isInList(leftBlock).0);
            }
            leftBlock
        }
    }
    /// Merges both blocks to one. The types of Blocks are ignored.
    unsafe fn mergeWithLeft(&self, leftBlock: *mut u8, middleBlock: *const u8) {
        #[cfg(feature = "condition")]
        {
            assert!(codeblock::isFree(leftBlock));
        }
        let leftEnd = leftBlock;
        let rightEnd = getRightMostEnd(middleBlock);
        let (codeBLockSize, _) = codeblock::getCodeBlockForInternalSize(
            leftEnd,
            rightEnd as usize - leftEnd as usize + 1,
            true,
        );
        copyCodeBlockToEnd(leftEnd, codeBLockSize);
        #[cfg(feature = "condition")]
        {
            assert!(codeblock::isFree(leftEnd));
            assert!(
                codeblock::readFromLeft(leftEnd)
                    == rightEnd as usize - leftEnd as usize - 2 * codeBLockSize + 1
            );
        }
    }
    //// Merges both blocks to one. The types of Blocks are ignored.
    unsafe fn mergeWithRight(&self, middleBlock: *const u8, rightBlock: *const u8) {
        #[cfg(feature = "condition")]
        {
            assert!(codeblock::isFree(rightBlock));
        }
        let leftEnd = middleBlock as *mut u8;
        let rightEnd = getRightMostEnd(rightBlock);
        let (codeBLockSize, _) = codeblock::getCodeBlockForInternalSize(
            leftEnd,
            rightEnd as usize - leftEnd as usize + 1,
            true,
        );
        copyCodeBlockToEnd(leftEnd, codeBLockSize);
        #[cfg(feature = "condition")]
        {
            assert!(codeblock::isFree(middleBlock));
            assert!(
                codeblock::readFromLeft(leftEnd)
                    == rightEnd as usize - leftEnd as usize - 2 * codeBLockSize + 1
            );
        }
    }
    /// Takes free space und cut the specified amount from space, starting at the left end. The new block has the adapted
    /// code blocks with the new size.
    /// #### freeSpace
    /// space to be cut
    /// #### bytesToCutOf
    /// amount of bytes to cut off from the left
    /// #### return
    /// null if the resulting block would be smaller than the smallest addressable block. A pointer to the
    /// resulting block otherwise
    fn cutLeftFromFreeSpace(&self, mut freeSpace: *mut u8, bytesToCutOf: usize) -> *mut u8 {
        #[cfg(feature = "condition")]
        {
            assert!(
                freeSpace >= self.startOfPage as *mut u8
                    && freeSpace < self.getStaticEnd() as *mut u8
            );
            assert!(getSize(freeSpace) >= bytesToCutOf);
        }
        if (getSize(freeSpace) as usize - bytesToCutOf) < SMALLEST_POSSIBLE_FREE_SPACE {
            #[cfg(feature = "condition")]
            {}
            return core::ptr::null_mut();
        } else {
            freeSpace =
                unsafe { pushBeginningRight(freeSpace, freeSpace.offset(bytesToCutOf as isize)) };
            #[cfg(feature = "condition")]
            {
                unsafe {
                    assert!(
                        getNext(freeSpace, self.startOfPage).is_null()
                            || (getNext(freeSpace, self.startOfPage)
                                >= self.startOfPage as *mut u8
                                && getNext(freeSpace, self.startOfPage)
                                    < self.staticEnd as *mut u8),
                    )
                };
                assert!(getSize(freeSpace) >= 6);
            }
            return freeSpace;
        }
    }
    /// Takes free space und cut the specified amount from space, starting at the right end. The new block has the adapted
    /// code blocks with the new size.
    /// #### freeSpace
    /// space to be cut
    /// #### bytesToCutOf
    /// amount of bytes to cut off from the left
    /// #### return
    /// null if the resulting block would be smaller than the smallest addressable block. A pointer to the
    /// resulting block otherwise
    fn cutRightFromFreeSpace(&self, freeSpace: *mut u8, bytesToCutOf: usize) -> *mut u8 {
        #[cfg(feature = "condition")]
        {
            assert!(getSize(freeSpace) >= bytesToCutOf); //there must be enough space in the freespace
            assert!(
                freeSpace >= self.startOfPage as *mut u8 && freeSpace < self.staticEnd as *mut u8
            );
            //the freespace must be in the page
        }
        if (getSize(freeSpace) - bytesToCutOf) < SMALLEST_POSSIBLE_FREE_SPACE {
            #[cfg(feature = "condition")]
            {
                //see if clause
            }
            return core::ptr::null_mut();
        } else {
            unsafe {
                pushEndLeft(
                    freeSpace,
                    getRightMostEnd(freeSpace).offset(-(bytesToCutOf as isize)),
                )
            };
            #[cfg(feature = "condition")]
            {
                unsafe {
                    //the next pointer must either be the invalid pointer or must point into the page
                    assert!(
                        getNext(freeSpace, self.startOfPage).is_null()
                            || (getNext(freeSpace, self.startOfPage)
                                >= self.startOfPage as *mut u8
                                && getNext(freeSpace, self.startOfPage)
                                    < self.staticEnd as *mut u8),
                    )
                };
                assert!(freeSpace >= self.startOfPage as *mut u8); //freespace must still be in the page
                assert!(getRightMostEnd(freeSpace) < self.staticEnd); //freespace may not go into the static area
            }
            freeSpace
        }
    }
    /// generates the first bucket entry
    /// #### return
    /// the first bucket entry
    unsafe fn generateFirstBucketEntry(startOfPage: *mut u8, end_of_page: *const u8) -> *mut u8 {
        let freeSpace = startOfPage;
        let (codeBlockSize, _) = codeblock::getCodeBlockForInternalSize(
            startOfPage,
            end_of_page as usize - startOfPage as usize,
            true,
        );
        copyCodeBlockToEnd(freeSpace, codeBlockSize);
        setNext(freeSpace, core::ptr::null(), startOfPage);
        return freeSpace;
    }
}
