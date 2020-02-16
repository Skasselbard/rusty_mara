use crate::code_block;
use crate::free_space::*;
use crate::globals::*;

pub struct BucketList {
    /// The array with the information of the dynamic free sections
    ///
    /// The space pointed to at the given index is the first one of the size class.
    ///
    /// Each index represent another size class. Increasing indices represent increasing size classes.
    bucket_list: [*mut u8; BUCKET_LIST_SIZE],

    start_of_page: *const u8,
}
impl BucketList {
    /// #### size
    /// #### return
    /// null if bucket is empty, the last element otherwise
    fn get_last_in_bucket(&self, size: usize) -> *const u8 {
        let mut current_element = self.bucket_list[size];
        if current_element == core::ptr::null_mut() {
            return current_element;
        }
        unsafe {
            while get_next(current_element, self.start_of_page) != core::ptr::null_mut() {
                current_element = get_next(current_element, self.start_of_page);
            }
        }
        current_element
    }
    /// #### index
    /// start index to search. The returned index will greater or equal to this index.
    /// #### return
    /// a bucket index with a non null entry. The index will always be >= the given index.
    #[inline]
    fn find_non_empty_bucket(&self, mut index: usize) -> usize {
        #[cfg(feature = "condition")]
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
        #[cfg(feature = "condition")]
        {
            assert!(!self.bucket_list[index].is_null() || index == BUCKET_LIST_SIZE - 1);
        }
        index
    }
    /// #### minimum_size
    /// count of bytes the space has to have at a minimum
    /// #### index
    /// the index of the bucket_list, where to search
    /// #### return
    /// null if no fitting space is found in the bucket, a free_space with a size greater than byte
    #[inline]
    unsafe fn find_fitting_space_in_bucket(&self, minimum_size: usize, index: usize) -> *mut u8 {
        #[cfg(feature = "condition")]
        {
            assert!(minimum_size > 0);
            assert!(index < BUCKET_LIST_SIZE);
        }
        let mut return_space = self.bucket_list[index];
        while return_space as usize > 0 && code_block::read_from_left(return_space) < minimum_size {
            return_space = get_next(return_space, self.start_of_page)
        }
        #[cfg(feature = "condition")]
        {
            assert!(
                return_space.is_null() || code_block::read_from_left(return_space) >= minimum_size
            );
        }
        return return_space;
    }

    /// Initializes a new bucket list.
    /// All entries are zeroed
    pub fn new(memory: [*mut u8; BUCKET_LIST_SIZE], start_of_page: *const u8) -> Self {
        let mut list = BucketList {
            bucket_list: memory,
            start_of_page,
        };
        for i in 0..BUCKET_LIST_SIZE {
            list.bucket_list[i] = core::ptr::null_mut();
        }
        list
    }
    /// This function does only give a free_space of the page. It does not alter the list itself.
    /// #### size_in_byte
    /// of the block of interest
    /// #### return
    /// null if there was no fitting space found. A pointer to the first free space in the list Otherwise.
    #[inline]
    pub unsafe fn get_free_space(&self, size_in_byte: usize) -> *mut u8 {
        #[cfg(feature = "condition")]
        {
            assert!(size_in_byte > 0);
        }
        let mut bucket_index = Self::lookup_bucket(size_in_byte);
        let mut return_space;
        loop {
            bucket_index = self.find_non_empty_bucket(bucket_index);
            return_space = self.find_fitting_space_in_bucket(size_in_byte, bucket_index);
            if !return_space.is_null() {
                bucket_index = bucket_index + 1
            }
            if !(return_space.is_null() && bucket_index < (BUCKET_LIST_SIZE - 1)) {
                break;
            }
        }
        if bucket_index == BUCKET_LIST_SIZE - 1 {
            return_space = self.find_fitting_space_in_bucket(size_in_byte, BUCKET_LIST_SIZE - 1);
        }
        #[cfg(feature = "condition")]
        {
            if !return_space.is_null() {
                assert!(code_block::read_from_left(return_space) >= size_in_byte);
                assert!(self.is_in_list(return_space).0);
            }
        }
        return return_space;
    }

    pub unsafe fn delete_from_list(&mut self, free_space: *mut u8) -> bool {
        #[cfg(feature = "condition")]
        {}
        let size = code_block::read_from_left(free_space);
        let (in_list, predecessor) = self.is_in_list(free_space);
        if in_list {
            if predecessor == core::ptr::null() {
                self.bucket_list[Self::lookup_bucket(size)] =
                    get_next(free_space, self.start_of_page);
            } else {
                set_next(
                    predecessor,
                    get_next(free_space, self.start_of_page),
                    self.start_of_page,
                );
            }
            #[cfg(feature = "condition")]
            {
                assert!(!self.is_in_list(free_space).0);
            }
            return true;
        }
        #[cfg(feature = "condition")]
        {
            assert!(false);
        }
        false
    }
    pub unsafe fn add_to_list(&mut self, free_space: *mut u8) -> bool {
        #[cfg(feature = "condition")]
        {
            assert!(free_space >= self.start_of_page as *mut u8);
            assert!(!self.is_in_list(free_space).0);
        }
        let size = code_block::read_from_left(free_space);
        let successor = self.bucket_list[Self::lookup_bucket(size)];
        set_next(free_space, successor, self.start_of_page);
        self.bucket_list[Self::lookup_bucket(size)] = free_space;
        #[cfg(feature = "condition")]
        {
            assert!(self.is_in_list(free_space).0);
        }
        true
    }

    fn set_start_of_page(&mut self, start_of_page: *const u8) {
        self.start_of_page = start_of_page;
    }

    pub fn get_from_bucket_list(&self, index: usize) -> *mut u8 {
        self.bucket_list[index]
    }

    /// Get the correct index in the bucket list for a block with the given memory size (without codeblocks)
    pub fn lookup_bucket(size: usize) -> usize {
        #[cfg(feature = "condition")]
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
    pub unsafe fn is_in_list(&self, free_space: *mut u8) -> (bool, *const u8) {
        #[cfg(feature = "condition")]
        {
            assert!(code_block::is_free(free_space));
        }
        let mut predecessor = core::ptr::null_mut();
        let mut current_element =
            self.bucket_list[Self::lookup_bucket(code_block::read_from_left(free_space))];
        if current_element.is_null() {
            return (false, core::ptr::null());
        }
        while !get_next(current_element, self.start_of_page).is_null()
            && current_element != free_space
        {
            predecessor = current_element;
            current_element = get_next(current_element, self.start_of_page);
        }
        if current_element != free_space {
            current_element = core::ptr::null_mut()
        }
        #[cfg(feature = "condition")]
        {
            assert!(
                current_element.is_null()
                    || free_space.is_null()
                    || predecessor.is_null()
                    || get_next(predecessor, self.start_of_page) == free_space,
            );
            assert!(current_element.is_null() || current_element == free_space);
        }
        (current_element.is_null(), predecessor)
    }
}
