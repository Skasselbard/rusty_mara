use crate::code_block;
use crate::free_space::*;
use crate::globals::*;
use crate::Page;
use crate::{AllocationData, MaraError};

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
    /// null if bucket is empty, the last element otherwise
    fn get_last_in_bucket(&self, size: usize) -> Result<AllocationData, MaraError> {
        let mut current_element = AllocationData::new();
        current_element.set_page(self.page);
        current_element.set_data_start(self.bucket_list[size]);
        if current_element.data_start()? == core::ptr::null_mut() {
            return Ok(current_element);
        }
        unsafe {
            while get_next(&current_element)? != core::ptr::null_mut() {
                current_element.set_data_start(get_next(&current_element)?);
            }
        }
        Ok(current_element)
    }
    /// #### index
    /// start index to search. The returned index will greater or equal to this index.
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
    /// #### minimum_size
    /// count of bytes the space has to have at a minimum
    /// #### index
    /// the index of the bucket_list, where to search
    /// #### return
    /// null if no fitting space is found in the bucket, a free_space with a size greater than byte
    #[inline]
    unsafe fn find_fitting_space_in_bucket(
        &self,
        minimum_size: usize,
        index: usize,
    ) -> Result<AllocationData, MaraError> {
        #[cfg(feature = "consistency-checks")]
        {
            assert!(minimum_size > 0);
            assert!(index < BUCKET_LIST_SIZE);
        }
        let mut return_alloc = AllocationData::new();
        return_alloc.set_page(self.page);
        return_alloc.set_data_start(self.bucket_list[index]);
        while !return_alloc.data_start()?.is_null()
            && code_block::read_from_left(return_alloc.data_start()?) < minimum_size
        {
            return_alloc.set_data_start(get_next(&return_alloc)?)
        }
        #[cfg(feature = "consistency-checks")]
        {
            assert!(
                return_alloc.data_start()?.is_null()
                    || code_block::read_from_left(return_alloc.data_start()?) >= minimum_size
            );
        }
        Ok(return_alloc)
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
    pub unsafe fn get_free_space(&self, alloc_data: &mut AllocationData) -> Result<(), MaraError> {
        #[cfg(feature = "consistency-checks")]
        {
            assert!(alloc_data.space_size()? > 0);
        }
        let mut bucket_index = Self::lookup_bucket(alloc_data.space_size()?);
        loop {
            bucket_index = self.find_non_empty_bucket(bucket_index);
            alloc_data.set_data_start(
                self.find_fitting_space_in_bucket(alloc_data.space_size()?, bucket_index)?
                    .data_start()?,
            );
            if !alloc_data.data_start()?.is_null() {
                bucket_index = bucket_index + 1
            }
            if !(alloc_data.data_start()?.is_null() && bucket_index < (BUCKET_LIST_SIZE - 1)) {
                break;
            }
        }
        if bucket_index == BUCKET_LIST_SIZE - 1 {
            alloc_data.set_data_start(
                self.find_fitting_space_in_bucket(alloc_data.space_size()?, BUCKET_LIST_SIZE - 1)?
                    .data_start()?,
            );
        }
        #[cfg(feature = "consistency-checks")]
        {
            if !alloc_data.data_start()?.is_null() {
                assert!(
                    code_block::read_from_left(alloc_data.data_start()?)
                        >= alloc_data.space_size()?
                );
                assert!(self.is_in_list(alloc_data)?.0);
            }
        }
        Ok(())
    }

    pub unsafe fn delete_from_list(
        &mut self,
        alloc_data: &mut AllocationData,
    ) -> Result<(), MaraError> {
        #[cfg(feature = "consistency-checks")]
        {}
        let (in_list, predecessor) = self.is_in_list(alloc_data)?;
        if in_list {
            if predecessor.data_start()?.is_null() {
                self.bucket_list[Self::lookup_bucket(alloc_data.data_size()?)] =
                    get_next(alloc_data)?;
            } else {
                set_next(alloc_data, get_next(alloc_data)?)?;
            }
            #[cfg(feature = "consistency-checks")]
            {
                assert!(!self.is_in_list(alloc_data)?.0);
            }
            Ok(())
        } else {
            Err(MaraError::AllocationNotFound)
        }
    }
    pub unsafe fn add_to_list(&mut self, alloc_data: &mut AllocationData) -> Result<(), MaraError> {
        #[cfg(feature = "consistency-checks")]
        {
            assert!(alloc_data.data_start()? >= (*self.page).start_of_page() as *mut u8);
            assert!(!self.is_in_list(alloc_data)?.0);
        }
        let size = code_block::read_from_left(alloc_data.data_start()?);
        let successor = self.bucket_list[Self::lookup_bucket(size)];
        set_next(alloc_data, successor)?;
        self.bucket_list[Self::lookup_bucket(size)] = alloc_data.data_start()?;
        #[cfg(feature = "consistency-checks")]
        {
            assert!(self.is_in_list(alloc_data)?.0);
        }
        Ok(())
    }

    pub fn get_from_bucket_list(&self, index: usize) -> *mut u8 {
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
    pub unsafe fn is_in_list(
        &self,
        alloc_data: &AllocationData,
    ) -> Result<(bool, AllocationData), MaraError> {
        #[cfg(feature = "consistency-checks")]
        {
            assert!(code_block::is_free(alloc_data.data_start()?));
        }
        let mut predecessor = core::ptr::null_mut();
        let mut current_element = AllocationData::new();
        current_element.set_page(alloc_data.page()?);
        current_element.set_data_start(
            self.bucket_list
                [Self::lookup_bucket(code_block::read_from_left(alloc_data.data_start()?))],
        );
        if current_element.data_start()?.is_null() {
            return Ok((false, current_element));
        }
        while !get_next(&current_element)?.is_null()
            && current_element.data_start()? != alloc_data.data_start()?
        {
            predecessor = current_element.data_start()?;
            current_element.set_data_start(get_next(&current_element)?);
        }
        if current_element.data_start()? != alloc_data.data_start()? {
            current_element.set_data_start(core::ptr::null_mut())
        }
        #[cfg(feature = "consistency-checks")]
        {
            assert!(
                current_element.data_start()?.is_null()
                    || alloc_data.data_start()?.is_null()
                    || predecessor.is_null() // || get_next(predecessor, self.start_of_page) == free_space,
            );
            assert!(
                current_element.data_start()?.is_null()
                    || current_element.data_start()? == alloc_data.data_start()?
            );
        }
        let in_list = !current_element.data_start()?.is_null();
        current_element.set_data_start(predecessor);
        Ok((in_list, current_element))
    }
}
