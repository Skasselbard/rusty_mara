use crate::code_block;
use crate::globals::*;
use crate::AllocationData;
use crate::Page;

pub struct BucketList {
    /// The array with the information of the dynamic free sections
    ///
    /// The space pointed to at the given index is the first one of the size class.
    ///
    /// Each index represent another size class. Increasing indices represent increasing size classes.
    bucket_list: [*mut u8; BUCKET_LIST_SIZE],

    page: *mut Page,
}
impl BucketList {
    /// #### index
    /// start index to search. The returned index will be greater or equal to this index.
    /// #### return
    /// a bucket index with a non null entry. The index will always be >= the given index.
    #[inline]
    fn find_non_empty_bucket(&self, mut index: usize) -> usize {
        #[cfg(feature = "consistency-checks")]
        {
            assert!(index < BUCKET_LIST_SIZE);
        }
        while self.bucket_list[index] == core::ptr::null_mut() {
            if index < BUCKET_LIST_SIZE - 1 {
                index = index + 1;
            } else {
                break;
            }
        }
        #[cfg(feature = "consistency-checks")]
        {
            assert!(!self.bucket_list[index].is_null() || index == BUCKET_LIST_SIZE - 1);
        }
        index
    }
    /// Greedy search in the bucket.
    /// Returns the first element that matches the size and ignores the actual size
    /// Null ``space.ptr`` if no fitting space is found in the bucket, a free_space with a size greater than byte
    #[inline]
    unsafe fn find_fitting_space_in_bucket(
        &self,
        minimum_size: usize,
        index: usize,
    ) -> AllocationData {
        #[cfg(feature = "consistency-checks")]
        {
            assert!(minimum_size > 0);
            assert!(index < BUCKET_LIST_SIZE);
        }
        let mut return_alloc = AllocationData::new();
        return_alloc.set_page(self.page);
        return_alloc
            .space
            .set_ptr(self.bucket_list[index] as *mut u8);
        // Search to the end of the bucket
        // Check if the adjacent code block encodes a fitting size
        while !return_alloc.space.ptr().is_null()
            && code_block::read_from_right(return_alloc.space.ptr().sub(1)).0 < minimum_size
        {
            return_alloc
                .space
                .set_ptr(return_alloc.space.read_next((*self.page).start_of_page()));
        }
        self.check_found(&return_alloc, minimum_size);
        return_alloc
    }

    /// Initializes a new bucket list.
    /// All entries are zeroed
    pub fn init(&mut self, page: *mut Page) {
        self.page = page;
        for i in 0..BUCKET_LIST_SIZE {
            self.bucket_list[i] = core::ptr::null_mut();
        }
    }
    /// This function does only give a free_space of the page. It does not alter the list itself.
    /// #### size_in_byte
    /// of the block of interest
    /// #### return
    /// null if there was no fitting space found. A pointer to the first free space in the list Otherwise.
    #[inline]
    pub unsafe fn get_free_space(&self, alloc_data: &mut AllocationData) {
        #[cfg(feature = "consistency-checks")]
        {
            assert!(alloc_data.space.size() > 0);
        }
        let mut bucket_index = Self::lookup_bucket(alloc_data.space.size());
        let mut found;
        loop {
            bucket_index = self.find_non_empty_bucket(bucket_index);
            found = self.find_fitting_space_in_bucket(alloc_data.space.size(), bucket_index);
            alloc_data.space.set_ptr(found.space.ptr());
            if !alloc_data.space.ptr().is_null() {
                bucket_index += 1;
            }
            if !(alloc_data.space.ptr().is_null() && bucket_index < (BUCKET_LIST_SIZE - 1)) {
                break;
            }
        }
        if bucket_index == BUCKET_LIST_SIZE - 1 {
            found =
                self.find_fitting_space_in_bucket(alloc_data.space.size(), BUCKET_LIST_SIZE - 1);
            alloc_data.space.set_ptr(found.space.ptr());
        }
        alloc_data.space.set_size(found.space.size());
        self.check_found(alloc_data, alloc_data.space.size());
    }

    pub unsafe fn remove(&mut self, alloc_data: &mut AllocationData) {
        let (in_list, mut predecessor) = self.is_in_list(alloc_data);
        if in_list {
            if predecessor.space.ptr().is_null() {
                self.bucket_list[Self::lookup_bucket(alloc_data.space.size())] =
                    alloc_data.space.next();
            } else {
                predecessor.space.set_next(alloc_data.space.next());
            }
            self.check_in_list(alloc_data, false);
        } else {
            panic!("Allocation not found");
        }
    }
    #[inline]
    /// A pointer on a space pointer.
    pub fn first_for_size(&self, alloc: &AllocationData) -> *mut u8 {
        self.bucket_list[Self::lookup_bucket(alloc.space.size())]
    }
    pub unsafe fn insert(&mut self, alloc_data: &mut AllocationData) {
        (*self.page).check_alloc_start(alloc_data);
        (*self.page).check_alloc_end(alloc_data);
        alloc_data.check_consistency();
        self.check_in_list(alloc_data, false);

        alloc_data.space.set_next(self.first_for_size(alloc_data));
        alloc_data.space.write_next(alloc_data.space.next());
        self.bucket_list[Self::lookup_bucket(alloc_data.space.size())] = alloc_data.space.ptr();

        #[cfg(feature = "consistency-checks")]
        {
            let mut alloc_post = alloc_data.clone();
            alloc_post.set_code_block_size(code_block::read_from_left(alloc_post.data_start()));
            self.check_in_list(&mut alloc_post, true);
        }
    }

    fn get_from_bucket_list(&self, index: usize) -> *mut u8 {
        self.bucket_list[index]
    }

    /// Get the correct index in the bucket list for a block with the given memory size (without codeblocks)
    pub fn lookup_bucket(size: usize) -> usize {
        #[cfg(feature = "consistency-checks")]
        {
            assert!(size > 0);
        }
        if size <= LAST_LINEAR_4_SCALING {
            return (size - 1) / 4;
        } else if size <= LAST_LINEAR_16_SCALING {
            return Self::lookup_bucket(LAST_LINEAR_4_SCALING)
                + 1
                + (size - LAST_LINEAR_4_SCALING - 1) / 16;
        } else if size <= LARGEST_BUCKET_SIZE {
            return Self::lookup_bucket(LAST_LINEAR_16_SCALING) + 1 + log2(size - 1)
                - log2(LAST_LINEAR_16_SCALING);
        } else {
            return BUCKET_LIST_SIZE - 1;
        }
    }
    /// #### free_space
    /// the Space to search for
    /// #### return
    /// is in list and the predecessor, if one is found(Output)
    pub unsafe fn is_in_list(&self, alloc_data: &AllocationData) -> (bool, AllocationData) {
        let mut current_element = AllocationData::new();
        current_element.set_page(alloc_data.page());
        current_element.space.set_next(core::ptr::null_mut());
        current_element
            .space
            .set_ptr(self.first_for_size(alloc_data));
        // empty bucket
        if current_element.space.ptr().is_null() {
            return (false, current_element);
        }
        loop {
            // iterate free space
            current_element.space.set_ptr(current_element.space.next());
            // cache next pointer fom new free space
            current_element.space.set_next(
                current_element
                    .space
                    .read_next((*self.page).start_of_page()),
            );
            // exit if next space is the searched one or if at end
            if current_element.space.next().is_null()
                || current_element.space.next() == alloc_data.space.ptr()
            {
                break;
            }
        }
        #[cfg(feature = "consistency-checks")]
        {
            assert!(
                current_element.space.next().is_null()
                    || alloc_data.space.ptr().is_null()
                    || current_element.space.ptr().is_null()
                    || current_element.space.next() == alloc_data.space.ptr(),
            );
        }
        // compute result
        let in_list = !current_element.space.next().is_null();
        (in_list, current_element)
    }

    /////////////////////////////////
    // Checks

    pub fn check_init(&self) {
        #[cfg(feature = "consistency-checks")]
        {
            // bucket list is empty
            for i in 0..(BUCKET_LIST_SIZE - 1) {
                if !(self.get_from_bucket_list(i).is_null()) {
                    dbg!(i);
                    dbg!(self.get_from_bucket_list(i));
                    unsafe { dbg!(*self.get_from_bucket_list(i)) };
                    panic!("bucket list not nulled")
                }
            }
            // The free space plus code blocks are as large as the page
            unsafe {
                let space = self.get_from_bucket_list(BUCKET_LIST_SIZE - 1) as *mut u8;
                let (memory_size, block) = code_block::read_from_right(space.sub(1));
                let block_size = code_block::get_block_size(block);
                if !(memory_size == (*self.page).page_size() - 2 * block_size) {
                    dbg!(space);
                    dbg!(block_size);
                    dbg!(memory_size);
                    dbg!((*self.page).page_size() - 2 * block_size);
                    panic!("space in bucket list is larger then the page")
                }
            }
        }
    }
    pub fn check_found(&self, alloc_data: &AllocationData, minimum_size: usize) {
        #[cfg(feature = "consistency-checks")]
        {
            // no space found
            if alloc_data.space.ptr().is_null() {
                return;
            }
            // check space size
            unsafe {
                let (memory_size, _) = code_block::read_from_right(alloc_data.space.ptr().sub(1));
                if memory_size < minimum_size {
                    dbg!(memory_size);
                    dbg!(minimum_size);
                    panic!("space in bucket list is smaller as expected")
                }
                if alloc_data.space.size() != memory_size {
                    dbg!(alloc_data.space.size());
                    dbg!(memory_size);
                    panic!("cached space size does not match calculated space size")
                }
            }
        }
    }
    pub fn check_in_list(&self, alloc_data: &AllocationData, expected: bool) {
        #[cfg(feature = "consistency-checks")]
        {
            unsafe {
                if self.is_in_list(alloc_data).0 != expected {
                    panic!(
                        "data is in list: {}\nexpected: {}",
                        self.is_in_list(alloc_data).0,
                        expected
                    )
                }
            }
        }
    }
    // pub fn check_insert(&self, alloc_data: &AllocationData) {
    //     #[cfg(feature = "consistency-checks")]
    //     {
    //         unsafe {
    //             if self.is_in_list(alloc_data).0 != expected {
    //                 panic!(
    //                     "data is in list: {}\nexpected: {}",
    //                     self.is_in_list(alloc_data).0,
    //                     expected
    //                 )
    //             }
    //         }
    //     }
    // }
}
