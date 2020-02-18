use crate::bucket_list::BucketList;
use crate::code_block;
use crate::free_space::*;
use crate::globals::*;
use crate::space::*;
use crate::{AllocationData, MaraError};

pub struct Page {
    /// Pointer to the first byte of the page
    start_of_page: *const u8,
    /// Pointer to the next page
    next_page: *mut Self,
    /// pointer to the leftmost byte of the static sector <br/>
    /// the rightmost byte is the last byte of the page
    end_of_page: *const u8,
    ///pointer to the rightmost allocated byte of the dynamic sector <br/>
    ///behind this pointer can only be an allocated chunk form the static
    ///sector. space between this pointer and the static_end pointer has to be free memory.
    dynamic_end: *const u8,
    bucket_list: BucketList,
}

impl Page {
    pub fn init(&mut self, page_memory: *mut u8, page_size: usize) -> Result<(), MaraError> {
        unsafe {
            let this = self as *mut Page;
            code_block::set_free(page_memory, true);
            self.bucket_list.init(this);
            let mut alloc_data = AllocationData::new();
            alloc_data.set_data_start(page_memory);
            alloc_data.set_data_end(page_memory.add(page_size));
            alloc_data.set_page(self);
            Self::generate_first_bucket_entry(&mut alloc_data)?;
            self.bucket_list.add_to_list(&mut alloc_data)?;
            #[cfg(feature = "consistency-checks")]
            {
                for i in 0..(BUCKET_LIST_SIZE - 1) {
                    assert!(self.bucket_list.get_from_bucket_list(i).is_null());
                }
                let block_size = code_block::get_block_size(
                    self.bucket_list.get_from_bucket_list(BUCKET_LIST_SIZE - 1),
                );
                assert!(
                    code_block::read_from_left(
                        self.bucket_list.get_from_bucket_list(BUCKET_LIST_SIZE - 1)
                    ) == (page_size - 2 * block_size),
                );
            }
            self.next_page = core::ptr::null_mut();
            self.start_of_page = page_memory;
            self.end_of_page = page_memory.offset(page_size as isize);
            self.dynamic_end = page_memory;
        }
        Ok(())
    }
    /// tries to reserve a dynamic block in this page, and returns it
    /// #### size_in_byte
    /// the size of the space requested
    /// #### return
    /// a pointer to the space, or nullptr if no space was found
    pub fn get_dynamic_block(&mut self, alloc_data: &mut AllocationData) -> Result<(), MaraError> {
        alloc_data.set_page(self);
        alloc_data.check_space_size(1, self.page_size())?;
        self.check_integrity()?;
        unsafe { self.bucket_list.get_free_space(alloc_data)? };
        if alloc_data.space()?.is_null() {
            self.check_integrity()?;
            return Err(MaraError::NoFittingSpace);
        } else {
            unsafe { self.bucket_list.delete_from_list(alloc_data)? };
            let did_cut = self.cut_left_from_free_space(
                alloc_data,
                alloc_data.space_size()?
                    + (2 * code_block::get_needed_code_block_size(alloc_data.space_size()?)),
            )?;
            if !did_cut {
                unsafe { self.bucket_list.add_to_list(alloc_data)? };
                unsafe { to_occupied(alloc_data)? };
            } else {
                //Edge Case: If the remaining space is too small to be used again, simply return a larger block
                unsafe { code_block::set_free(alloc_data.data_start()?, false) };
                unsafe { copy_code_block_to_end(alloc_data)? };
            }

            if alloc_data.data_end()? as usize > self.dynamic_end as usize {
                self.dynamic_end = alloc_data.data_end()?;
            }
        }
        self.check_integrity()?;
        self.check_alloc(alloc_data)?;
        alloc_data.check_space()?;
        alloc_data.check_consistency()?;
        Ok(())
    }
    /// #### return the next page in the ring storage
    pub fn get_next_page(&self) -> *mut Self {
        self.next_page
    }
    /// sets the next page
    /// #### next_page
    /// the next page
    pub fn set_next_page(&mut self, next_page: *mut Self) {
        if next_page != core::ptr::null_mut() {}
        self.next_page = next_page;
    }
    /// #### first_byte
    /// a pointer to the block of interest
    /// #### return
    /// true if the pointer is in between the start of page and the left most byte of the static sector.
    /// false otherwise. Blocks in the static sector CANNOT be detected with this function.
    pub fn block_is_in_space(&self, first_byte: *const u8) -> bool {
        self.start_of_page <= first_byte && first_byte < self.end_of_page
    }
    /// deletes a reserved block
    /// #### first_byte
    /// the first byte of the block
    /// #### return
    /// true if successful, false otherwise
    pub fn delete_block(&mut self, alloc_data: &mut AllocationData) -> Result<(), MaraError> {
        alloc_data.set_page(self);
        self.check_integrity()?;
        let (memory_block_size, code_block_start) =
            unsafe { code_block::read_from_right(alloc_data.data_start()?.offset(-1)) };
        alloc_data.set_data_start(code_block_start as *mut u8);
        alloc_data.set_space_size(memory_block_size);
        let code_block_size = unsafe { code_block::get_block_size(code_block_start) };
        #[cfg(feature = "statistic")]
        {
            Statistic::freeDynamic(memory_block_size, first_byte);
        }
        if (code_block_start as usize + (2 * code_block_size) + memory_block_size)
            > self.end_of_page as usize
        {
            panic!("code block reaches into static space")
        }
        let mut left_neighbor = AllocationData::new();
        left_neighbor.set_page(alloc_data.page()?);
        let mut right_neighbor = AllocationData::new();
        right_neighbor.set_page(alloc_data.page()?);
        right_neighbor.set_data_start(unsafe {
            code_block_start.add(2 * code_block_size + memory_block_size) as *mut u8
        });
        if right_neighbor.data_start()? as usize > self.end_of_page as usize {
            return Err(MaraError::PageOverflow);
        }
        if self.start_of_page < code_block_start {
            left_neighbor.set_data_start(get_left_neighbor(alloc_data)? as *mut u8);
        }
        if !left_neighbor.data_start().is_err() && !code_block::is_free(left_neighbor.data_start()?)
        {
            left_neighbor.set_data_start(core::ptr::null_mut());
        }
        if !right_neighbor.data_start()?.is_null()
            && (right_neighbor.data_start()? as usize >= self.end_of_page as usize
                || !code_block::is_free(right_neighbor.data_start()?))
        {
            right_neighbor.set_data_start(core::ptr::null_mut());
        }
        unsafe { self.merge_free_space(&mut left_neighbor, alloc_data, &mut right_neighbor)? };
        #[cfg(feature = "consistency-checks")]
        {
            unsafe {
                assert!(
                    (left_neighbor.data_start()?.is_null()
                        && self.bucket_list.is_in_list(alloc_data)?.0
                        && code_block::is_free(code_block_start))
                        // || (self.bucket_list.is_in_list(left_neighbor).0
                            // && code_block::is_free(left_neighbor)),
                )
            };
        }
        self.check_integrity()?;
        Ok(())
    }
    /// #### return
    /// a pointer to the first byte in the page
    pub fn start_of_page(&self) -> *const u8 {
        self.start_of_page
    }
    /// #### return
    /// the dynamic end
    pub fn dynamic_end(&self) -> *const u8 {
        self.dynamic_end
    }
    /// #### return
    /// the bucket list
    pub fn bucket_list(&self) -> &BucketList {
        &self.bucket_list
    }
    /// Merges up to three blocks into one Block of free Space.
    /// Only free blocks are merged.
    /// The bucket_list will be updated accordingly<br/>
    /// WARNING: the blocks have to be adjacent to each other. Merging distant blocks will cause undefined behavior.
    /// Probably causing the world as we know it, to cease to exist!
    /// #### left_block
    /// left_block to be merged. Ignored if null
    /// #### middle_block
    /// middle Block to be merged
    /// #### right_block
    /// right Block to be merged. Ignored if null
    /// #### return
    /// the new block of free space
    #[inline]
    unsafe fn merge_free_space(
        &mut self,
        l_alloc: &mut AllocationData,
        alloc_data: &mut AllocationData,
        r_alloc: &mut AllocationData,
    ) -> Result<(), MaraError> {
        alloc_data.check_consistency()?;
        #[cfg(feature = "consistency-checks")]
        {
            assert!(r_alloc.data_start()?.is_null() || self.bucket_list.is_in_list(r_alloc)?.0);
            assert!(l_alloc.data_start()?.is_null() || self.bucket_list.is_in_list(l_alloc)?.0);
        }
        if l_alloc.data_start()?.is_null() {
            if !r_alloc.data_start()?.is_null() {
                self.bucket_list.delete_from_list(r_alloc)?;
                self.merge_with_right(alloc_data, r_alloc)?;
            }
            code_block::set_free(alloc_data.data_start()?, true);
            alloc_data.set_code_block_size(code_block::get_block_size(alloc_data.data_start()?));
            copy_code_block_to_end(alloc_data)?;
            self.bucket_list.add_to_list(alloc_data)?;
            alloc_data.check_consistency()?;
            #[cfg(feature = "consistency-checks")]
            {
                assert!(self.bucket_list.is_in_list(alloc_data)?.0);
            }
        } else {
            if !r_alloc.data_start()?.is_null() {
                self.bucket_list.delete_from_list(r_alloc)?;
                self.merge_with_right(alloc_data, r_alloc)?;
            }
            self.bucket_list.delete_from_list(l_alloc)?;

            self.merge_with_left(l_alloc.data_start()?, alloc_data)?;
            code_block::set_free(l_alloc.data_start()?, true);
            l_alloc.set_code_block_size(code_block::get_block_size(l_alloc.data_start()?));
            copy_code_block_to_end(l_alloc)?;
            self.bucket_list.add_to_list(l_alloc)?;
            alloc_data.check_consistency()?;
            #[cfg(feature = "consistency-checks")]
            {
                assert!(self.bucket_list.is_in_list(l_alloc)?.0);
            }
        }
        Ok(())
    }
    /// Merges both blocks to one. The types of Blocks are ignored.
    #[inline]
    unsafe fn merge_with_left(
        &self,
        left_block: *mut u8,
        alloc_data: &mut AllocationData,
    ) -> Result<(), MaraError> {
        #[cfg(feature = "consistency-checks")]
        {
            assert!(code_block::is_free(left_block));
        }
        alloc_data.set_data_start(left_block);
        //let right_end = get_right_most_end(middle_block);
        let (code_block_size, _) = code_block::get_code_block_for_internal_size(
            left_block,
            alloc_data.data_size()? as usize + 1,
            true,
        );
        alloc_data.set_code_block_size(code_block_size);
        copy_code_block_to_end(alloc_data)?;
        #[cfg(feature = "consistency-checks")]
        {
            assert!(code_block::is_free(left_block));
            assert!(
                code_block::read_from_left(left_block)
                    == alloc_data.data_end()? as usize - left_block as usize - 2 * code_block_size
                        + 1
            );
        }
        Ok(())
    }
    //// Merges both blocks to one. The types of Blocks are ignored.
    #[inline]
    unsafe fn merge_with_right(
        &self,
        alloc_data: &mut AllocationData,
        r_alloc: &mut AllocationData,
    ) -> Result<(), MaraError> {
        #[cfg(feature = "consistency-checks")]
        {
            assert!(code_block::is_free(r_alloc.data_start()?));
        }
        alloc_data.set_data_end(r_alloc.data_end()?);
        let (code_block_size, _) = code_block::get_code_block_for_internal_size(
            alloc_data.data_start()?,
            alloc_data.data_size()? as usize + 1,
            true,
        );
        alloc_data.set_space_size(code_block_size);
        copy_code_block_to_end(alloc_data)?;
        #[cfg(feature = "consistency-checks")]
        {
            assert!(code_block::is_free(alloc_data.data_start()?));
            assert!(
                code_block::read_from_left(alloc_data.data_start()?)
                    == alloc_data.data_size()? - 2 * code_block_size + 1
            );
        }
        Ok(())
    }
    /// Takes free space und cut the specified amount from space, starting at the left end. The new block has the adapted
    /// code blocks with the new size.
    /// #### free_space
    /// space to be cut
    /// #### bytesToCutOf
    /// amount of bytes to cut off from the left
    /// #### return
    /// false if the resulting block would be smaller than the smallest addressable block. True otherwise
    #[inline]
    fn cut_left_from_free_space(
        &self,
        alloc_data: &mut AllocationData,
        bytes_to_cut_of: usize,
    ) -> Result<bool, MaraError> {
        #[cfg(feature = "consistency-checks")]
        {
            assert!(
                alloc_data.data_start()? >= self.start_of_page as *mut u8
                    && alloc_data.data_start()? < self.end_of_page as *mut u8
            );
            assert!(alloc_data.data_size()? >= bytes_to_cut_of);
        }
        if (alloc_data.data_size()? as usize - bytes_to_cut_of) < SMALLEST_POSSIBLE_FREE_SPACE {
            #[cfg(feature = "consistency-checks")]
            {}
            Ok(false)
        } else {
            unsafe {
                push_beginning_right(
                    alloc_data,
                    alloc_data.data_start()?.offset(bytes_to_cut_of as isize),
                )?
            };
            #[cfg(feature = "consistency-checks")]
            {
                unsafe {
                    assert!(
                        get_next(alloc_data)?.is_null()
                            || (get_next(alloc_data)? >= self.start_of_page as *mut u8
                                && get_next(alloc_data)? < self.end_of_page as *mut u8),
                    )
                };
                assert!(alloc_data.data_size()? >= 6);
            }
            Ok(true)
        }
    }
    /// Takes free space und cut the specified amount from space, starting at the right end. The new block has the adapted
    /// code blocks with the new size.
    /// #### free_space
    /// space to be cut
    /// #### bytesToCutOf
    /// amount of bytes to cut off from the left
    /// #### return
    /// false if the resulting block would be smaller than the smallest addressable
    /// block. True resulting block otherwise
    #[inline]
    fn cut_right_from_free_space(
        &self,
        alloc_data: &mut AllocationData,
        bytes_to_cut_of: usize,
    ) -> Result<bool, MaraError> {
        #[cfg(feature = "consistency-checks")]
        {
            assert!(alloc_data.data_size()? >= bytes_to_cut_of); //there must be enough space in the freespace
            assert!(
                alloc_data.data_start()? >= self.start_of_page as *mut u8
                    && alloc_data.data_start()? < self.end_of_page as *mut u8
            );
            //the freespace must be in the page
        }
        if (alloc_data.data_size()? - bytes_to_cut_of) < SMALLEST_POSSIBLE_FREE_SPACE {
            #[cfg(feature = "consistency-checks")]
            {
                //see if clause
            }
            Ok(false)
        } else {
            unsafe { push_end_left(alloc_data, alloc_data.data_end()?.sub(bytes_to_cut_of)) };
            #[cfg(feature = "consistency-checks")]
            {
                unsafe {
                    //the next pointer must either be the invalid pointer or must point into the page
                    assert!(
                        get_next(alloc_data)?.is_null()
                            || (get_next(alloc_data)? >= self.start_of_page as *mut u8
                                && get_next(alloc_data)? < self.end_of_page as *mut u8),
                    )
                };
                assert!(alloc_data.data_start()? >= self.start_of_page as *mut u8); //freespace must still be in the page
                assert!(alloc_data.data_end()? < self.end_of_page as *mut u8); //freespace may not go into the static area
            }
            Ok(true)
        }
    }
    /// generates the first bucket entry
    /// #### return
    /// the first bucket entry
    #[inline]
    unsafe fn generate_first_bucket_entry(
        alloc_data: &mut AllocationData,
    ) -> Result<(), MaraError> {
        let (code_block_size, _) = code_block::get_code_block_for_internal_size(
            alloc_data.data_start()?,
            alloc_data.data_size()?,
            true,
        );
        alloc_data.set_code_block_size(code_block_size);
        copy_code_block_to_end(alloc_data)?;
        set_next(alloc_data, core::ptr::null())?;
        Ok(())
    }

    fn page_size(&self) -> usize {
        self.end_of_page as usize - self.start_of_page as usize
    }

    fn check_integrity(&self) -> Result<(), MaraError> {
        #[cfg(feature = "consistency-checks")]
        {
            if self.start_of_page as usize > self.end_of_page as usize {
                dbg!(self.start_of_page);
                dbg!(self.end_of_page);
                return Err(MaraError::InconsistentPage);
            }
            if self.end_of_page as usize > self.dynamic_end as usize {
                dbg!(self.start_of_page);
                dbg!(self.dynamic_end);
                return Err(MaraError::PageOverflow);
            }
        }
        Ok(())
    }

    fn check_alloc(&self, alloc_data: &AllocationData) -> Result<(), MaraError> {
        #[cfg(feature = "consistency-checks")]
        {
            unsafe {
                if alloc_data.data_start()? as usize >= self.start_of_page as usize
                    && alloc_data.space()? as usize > self.start_of_page as usize
                    && alloc_data.data_end()? as usize <= self.end_of_page as usize
                    && (alloc_data.space()?.add(alloc_data.space_size()?) as usize)
                        < self.end_of_page as usize
                {
                    dbg!(self.start_of_page);
                    dbg!(self.end_of_page);
                    dbg!(alloc_data.data_start()?);
                    dbg!(alloc_data.space()?);
                    dbg!(alloc_data.space()?.add(alloc_data.space_size()?));
                    dbg!(alloc_data.data_end()?);
                    return Err(MaraError::InconsistentAllocationData);
                }
            }
        }
        Ok(())
    }
}
