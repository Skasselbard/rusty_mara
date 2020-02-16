use crate::codeblock;
use crate::freespace::*;
use crate::globals::*;

pub struct BucketList {
    /// The array with the information of the dynamic free sections
    ///
    /// The space pointed to at the given index is the first one of the size class.
    ///
    /// Each index represent another size class. Increasing indices represent increasing size classes.
    bucketList: [*mut u8; blSize],

    startOfPage: *const u8,
}
impl BucketList {
    /// #### size
    /// #### return
    /// null if bucket is empty, the last element otherwise
    fn getLastInBucket(&self, size: usize) -> *const u8 {
        let mut currentElement = self.bucketList[size];
        if currentElement == core::ptr::null_mut() {
            return currentElement;
        }
        unsafe {
            while getNext(currentElement, self.startOfPage) != core::ptr::null_mut() {
                currentElement = getNext(currentElement, self.startOfPage);
            }
        }
        currentElement
    }
    /// #### index
    /// start index to search. The returned index will greater or equal to this index.
    /// #### return
    /// a bucket index with a non null entry. The index will always be >= the given index.
    fn findNonEmptyBucket(&self, mut index: usize) -> usize {
        #[cfg(feature = "condition")]
        {
            assert!(index < blSize);
        }
        while self.bucketList[index] == core::ptr::null_mut() {
            if index < blSize - 1 {
                index = index + 1;
            } else {
                break;
            }
        }
        #[cfg(feature = "condition")]
        {
            assert!(!self.bucketList[index].is_null() || index == blSize - 1);
        }
        index
    }
    /// #### minimumSize
    /// count of bytes the space has to have at a minimum
    /// #### index
    /// the index of the bucketlist, where to search
    /// #### return
    /// null if no fitting space is found in the bucket, a freeSpace with a size greater than byte
    unsafe fn findFittingSpaceInBucket(&self, minimumSize: usize, index: usize) -> *mut u8 {
        #[cfg(feature = "condition")]
        {
            assert!(minimumSize > 0);
            assert!(index < blSize);
        }
        let mut returnSpace = self.bucketList[index];
        while returnSpace as usize > 0 && codeblock::readFromLeft(returnSpace) < minimumSize {
            returnSpace = getNext(returnSpace, self.startOfPage);
        }
        #[cfg(feature = "condition")]
        {
            assert!(returnSpace.is_null() || codeblock::readFromLeft(returnSpace) >= minimumSize);
        }
        return returnSpace;
    }

    /// Initializes a new bucket list.
    /// All entries are zeroed
    pub fn new(memory: [*mut u8; blSize], startOfPage: *const u8) -> Self {
        let mut list = Self {
            bucketList: memory,
            startOfPage,
        };
        for i in 0..blSize {
            list.bucketList[i] = core::ptr::null_mut();
        }
        list
    }
    /// This function does only give a freeSpace of the page. It does not alter the list itself.
    /// #### sizeInByte
    /// of the block of interest
    /// #### return
    /// null if there was no fitting space found. A pointer to the first free space in the list Otherwise.
    pub unsafe fn getFreeSpace(&self, sizeInByte: usize) -> *mut u8 {
        #[cfg(feature = "condition")]
        {
            assert!(sizeInByte > 0);
        }
        let mut bucketIndex = Self::lookupBucket(sizeInByte);
        let mut returnSpace;
        loop {
            bucketIndex = self.findNonEmptyBucket(bucketIndex);
            returnSpace = self.findFittingSpaceInBucket(sizeInByte, bucketIndex);
            if !returnSpace.is_null() {
                bucketIndex = bucketIndex + 1
            }
            if !(returnSpace.is_null() && bucketIndex < (blSize - 1)) {
                break;
            }
        }
        if bucketIndex == blSize - 1 {
            returnSpace = self.findFittingSpaceInBucket(sizeInByte, blSize - 1);
        }
        #[cfg(feature = "condition")]
        {
            if !returnSpace.is_null() {
                assert!(codeblock::readFromLeft(returnSpace) >= sizeInByte);
                assert!(self.isInList(returnSpace).0);
            }
        }
        return returnSpace;
    }

    pub unsafe fn deleteFromList(&mut self, freeSpace: *mut u8) -> bool {
        #[cfg(feature = "condition")]
        {}
        let size = codeblock::readFromLeft(freeSpace);
        let (inList, predecessor) = self.isInList(freeSpace);
        if inList {
            if predecessor == core::ptr::null() {
                self.bucketList[Self::lookupBucket(size)] = getNext(freeSpace, self.startOfPage);
            } else {
                setNext(
                    predecessor,
                    getNext(freeSpace, self.startOfPage),
                    self.startOfPage,
                );
            }
            #[cfg(feature = "condition")]
            {
                assert!(!self.isInList(freeSpace).0);
            }
            return true;
        }
        #[cfg(feature = "condition")]
        {
            assert!(false);
        }
        false
    }
    pub unsafe fn addToList(&mut self, freeSpace: *mut u8) -> bool {
        #[cfg(feature = "condition")]
        {
            assert!(freeSpace >= self.startOfPage as *mut u8);
            assert!(!self.isInList(freeSpace).0);
        }
        let size = codeblock::readFromLeft(freeSpace);
        let successor = self.bucketList[Self::lookupBucket(size)];
        setNext(freeSpace, successor, self.startOfPage);
        self.bucketList[Self::lookupBucket(size)] = freeSpace;
        #[cfg(feature = "condition")]
        {
            assert!(self.isInList(freeSpace).0);
        }
        true
    }

    fn setStartOfPage(&mut self, startOfPage: *const u8) {
        self.startOfPage = startOfPage;
    }

    pub fn getFromBucketList(&self, index: usize) -> *mut u8 {
        self.bucketList[index]
    }

    /// Get the correct index in the bucket list for a block with the given memory size (without codeblocks)
    pub fn lookupBucket(size: usize) -> usize {
        #[cfg(feature = "condition")]
        {
            assert!(size > 0);
        }
        if size <= lastLinear4Scaling {
            return (size - 1) / 4;
        } else if size <= lastLinear16Scaling {
            return Self::lookupBucket(lastLinear4Scaling)
                + 1
                + (size - lastLinear4Scaling - 1) / 16;
        } else if size <= largestBucketSize {
            return Self::lookupBucket(lastLinear16Scaling) + 1 + log2(size - 1)
                - log2(lastLinear16Scaling);
        } else {
            return blSize - 1;
        }
    }
    /// #### freeSpace
    /// the Space to search for
    /// #### return
    /// is in list and the predecessor, if one is found(Output)
    pub unsafe fn isInList(&self, freeSpace: *mut u8) -> (bool, *const u8) {
        #[cfg(feature = "condition")]
        {
            assert!(codeblock::isFree(freeSpace));
        }
        let mut predecessor = core::ptr::null_mut();
        let mut currentElement =
            self.bucketList[Self::lookupBucket(codeblock::readFromLeft(freeSpace))];
        if currentElement.is_null() {
            return (false, core::ptr::null());
        }
        while !getNext(currentElement, self.startOfPage).is_null() && currentElement != freeSpace {
            predecessor = currentElement;
            currentElement = getNext(currentElement, self.startOfPage);
        }
        if currentElement != freeSpace {
            currentElement = core::ptr::null_mut()
        }
        #[cfg(feature = "condition")]
        {
            assert!(
                currentElement.is_null()
                    || freeSpace.is_null()
                    || predecessor.is_null()
                    || getNext(predecessor, self.startOfPage) == freeSpace,
            );
            assert!(currentElement.is_null() || currentElement == freeSpace);
        }
        (currentElement.is_null(), predecessor)
    }
}
