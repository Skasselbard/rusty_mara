use crate::code_block;
use crate::globals::*;
use crate::space::Space;
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
        while self.get(index).is_none() {
            if index < BUCKET_LIST_SIZE - 1 {
                index += 1;
            } else {
                break;
            }
        }
        #[cfg(feature = "consistency-checks")]
        {
            assert!(!self.get(index).is_none() || index == BUCKET_LIST_SIZE - 1);
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
    ) -> Option<Space> {
        #[cfg(feature = "consistency-checks")]
        {
            assert!(minimum_size > 0);
            assert!(index < BUCKET_LIST_SIZE);
        }
        let mut space = self.get(index);
        space = if let Some(mut space) = space {
            // Search to the end of the bucket
            // Check if the adjacent code block encodes a fitting size
            while !space.ptr().is_null()
                && code_block::read_from_right(space.ptr().sub(1)).0 < minimum_size
            {
                space.set_ptr(space.read_next((*self.page).start_of_page()))
            }
            // check the result
            match space.ptr().is_null() {
                true => None,
                false => Some(space),
            }
        } else {
            None
        };
        self.check_found(&space, minimum_size);
        space
    }

    /// Initializes a new bucket list.
    /// All entries are zeroed
    #[inline]
    pub fn init(&mut self, page: *mut Page) {
        self.page = page;
        for i in 0..BUCKET_LIST_SIZE {
            self.bucket_list[i] = core::ptr::null_mut();
        }
    }
    /// This function does only give a free_space of the page. It does not alter the list itself.
    /// Non if no space was found
    #[inline]
    pub unsafe fn get_free_space(&self, alloc_data: &mut AllocationData) -> Option<Space> {
        #[cfg(feature = "consistency-checks")]
        {
            assert!(alloc_data.space.size() > 0);
        }
        let mut bucket_index = Self::lookup_bucket(alloc_data.space.size());
        let mut found;
        loop {
            bucket_index = self.find_non_empty_bucket(bucket_index);
            found = self.find_fitting_space_in_bucket(alloc_data.space.size(), bucket_index);
            match found {
                None => bucket_index += 1,
                Some(_) => {}
            }
            if bucket_index == (BUCKET_LIST_SIZE - 1) {
                break;
            }
        }
        self.check_found(&found, alloc_data.space.size());
        found
    }
    /// removes ``space`` from the bucket list
    /// panics if it was not found
    pub unsafe fn remove(&mut self, space: &Space) {
        let (in_list, predecessor) = self.is_in_list(&space);
        if in_list {
            // alloc data is not the first element in the bucket
            if let Some(mut predecessor) = predecessor {
                predecessor.set_next(space.next().ptr());
            }
            // alloc data is the first element in the bucket
            else {
                self.bucket_list[Self::lookup_bucket(space.size())] = space.next().ptr();
            }
            self.check_in_list(space, false);
        } else {
            panic!("Allocation not found");
        }
    }
    /// A pointer on a space pointer from the bucket with the given index
    #[inline]
    fn get(&self, index: usize) -> Option<Space> {
        match self.bucket_list[index] {
            ptr if ptr.is_null() => None,
            ptr => {
                let mut space = Space::new();
                space.set_ptr(ptr);
                Some(space)
            }
        }
    }
    /// A pointer on a space pointer with a size grater or equal to ``size``
    #[inline]
    pub fn first_for_size(&self, size: usize) -> Option<Space> {
        match self.bucket_list[Self::lookup_bucket(size)] {
            ptr if ptr.is_null() => None,
            ptr => {
                let mut space = Space::new();
                space.set_ptr(ptr);
                Some(space)
            }
        }
    }
    pub unsafe fn insert(&mut self, space: &mut Space) {
        self.check_in_list(space, false);

        space.set_next(match self.first_for_size(space.size()) {
            Some(space) => space.ptr(),
            None => core::ptr::null_mut(),
        });
        space.write_next(space.next().ptr());
        self.bucket_list[Self::lookup_bucket(space.size())] = space.ptr();

        self.check_in_list(space, true);
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
    #[inline]
    pub unsafe fn is_in_list(&self, space: &Space) -> (bool, Option<Space>) {
        if let Some(mut current_element) = self.first_for_size(space.size()) {
            // first element is the searched one
            if current_element.ptr() == space.ptr() {
                return (true, None);
            }
            // empty bucket: return not found
            if current_element.ptr().is_null() {}
            let start_of_page = (*self.page).start_of_page();
            current_element.set_next(current_element.read_next(start_of_page));
            while !current_element.next().ptr().is_null()
                && current_element.next().ptr() != space.ptr()
            {
                // iterate free space
                current_element.set_ptr(current_element.next().ptr());
                // cache next pointer fom new free space
                current_element.set_next(current_element.read_next(start_of_page));
            }
            #[cfg(feature = "consistency-checks")]
            {
                assert!(
                    current_element.next().ptr().is_null()
                        || space.ptr().is_null()
                        || current_element.ptr().is_null()
                        || current_element.next().ptr() == space.ptr(),
                );
            }
            // compute result
            let in_list = !current_element.next().ptr().is_null();
            (in_list, Some(current_element))
        }
        // bucket is empty
        else {
            (false, None)
        }
    }

    /////////////////////////////////
    // Checks

    pub fn check_init(&self) {
        #[cfg(feature = "consistency-checks")]
        {
            // bucket list is empty
            for i in 0..(BUCKET_LIST_SIZE - 1) {
                if !(self.bucket_list[i].is_null()) {
                    dbg!(i);
                    dbg!(self.bucket_list[i]);
                    panic!("bucket list not nulled")
                }
            }
            // The free space plus code blocks are as large as the page
            unsafe {
                let space = self
                    .get(BUCKET_LIST_SIZE - 1)
                    .expect("Bucket is empty")
                    .ptr() as *mut u8;
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
    pub fn check_found(&self, space: &Option<Space>, minimum_size: usize) {
        #[cfg(feature = "consistency-checks")]
        {
            match space {
                None => {}
                Some(space) =>
                // check space size
                unsafe {
                    let (memory_size, _) = code_block::read_from_right(space.ptr().sub(1));
                    if memory_size < minimum_size {
                        dbg!(memory_size);
                        dbg!(minimum_size);
                        panic!("space in bucket list is smaller as expected")
                    }
                }
            }
        }
    }
    pub fn check_in_list(&self, space: &Space, expected: bool) {
        #[cfg(feature = "consistency-checks")]
        {
            unsafe {
                if self.is_in_list(&space).0 != expected {
                    panic!(
                        "data is in list: {}\nexpected: {}",
                        self.is_in_list(&space).0,
                        expected
                    )
                }
            }
        }
    }
}
