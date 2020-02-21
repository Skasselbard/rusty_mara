use crate::bucket_list::BucketList;
use crate::code_block;
use crate::free_space::*;
use crate::globals::*;
use crate::space::*;
use crate::AllocationData;
use core::mem::size_of;
use core::ops::*;

pub struct Page {
    /// Pointer to the first byte of the page
    start_of_page: *const u8,
    /// Pointer to the next page
    next_page: *mut Self,
    /// pointer to the leftmost byte of the static sector <br/>
    /// the rightmost byte is the last byte of the page
    end_of_page: *const u8,
    bucket_list: BucketList,
}

impl Page {
    pub fn init(&mut self, page_memory: *mut u8, page_size: usize) {
        unsafe {
            let this = self as *mut Page;
            self.next_page = core::ptr::null_mut();
            self.start_of_page = page_memory;
            self.end_of_page = page_memory.add(page_size);
            code_block::set_free(page_memory, true);
            self.bucket_list.init(this);
            let mut alloc_data = AllocationData::new();
            alloc_data.set_data_start(page_memory);
            alloc_data.set_data_end(page_memory.add(page_size).sub(1));
            alloc_data.set_page(self);
            Self::generate_first_bucket_entry(&mut alloc_data);
            alloc_data.set_code_block_size(code_block::get_block_size(alloc_data.data_start()));
            alloc_data.set_space(alloc_data.data_start().add(alloc_data.code_block_size()));
            alloc_data.set_space_size(page_size - 2 * alloc_data.code_block_size());
            self.bucket_list.add_to_list(&mut alloc_data);

            self.check_integrity();
            self.bucket_list().check_init();
            alloc_data.check_left_free(true);
            alloc_data.check_consistency();
            alloc_data.check_space();
            alloc_data.check_data_size(page_size, page_size);
        }
    }
    /// generates the first bucket entry
    /// #### return
    /// the first bucket entry
    #[inline]
    unsafe fn generate_first_bucket_entry(alloc_data: &mut AllocationData) {
        let code_block_size = code_block::generate_code_block_for_internal_size(
            alloc_data.data_start(),
            alloc_data.calculate_data_size(),
            true,
        );
        alloc_data.set_code_block_size(code_block_size);
        copy_code_block_to_end(alloc_data);
        set_next(alloc_data, core::ptr::null_mut());
    }
    #[inline]
    pub fn page_size(&self) -> usize {
        self.end_of_page as usize - self.start_of_page as usize
    }
    /// tries to reserve a dynamic block in this page, and returns it
    pub fn get_dynamic_block(&mut self, alloc_data: &mut AllocationData) {
        unsafe {
            alloc_data.set_page(self);
            alloc_data.check_space_size(1, self.page_size());
            self.check_integrity();

            let mut free_alloc = alloc_data.clone();
            self.bucket_list.get_free_space(&mut free_alloc);
            if free_alloc.space().is_null() {
                self.check_integrity();
                return;
            } else {
                // Remove this free space from list
                // the remaining space will be added again later
                self.bucket_list.delete_from_list(&mut free_alloc);
                // Calculate where the allocation starts
                // It will be at the beginning of the found free space
                alloc_data.set_data_start(free_alloc.space().sub(
                    code_block::get_needed_code_block_size(free_alloc.space_size()),
                ));
                // next has to be cached
                free_alloc.set_next_pointer(get_next(&free_alloc));
                // cache the end of the free space for later
                free_alloc.set_data_end(
                    free_alloc
                        .space()
                        .add(free_alloc.space_size())
                        .add(code_block::get_block_size(alloc_data.data_start()))
                        .sub(1),
                );
                // split the free space in two
                self.split_free_space(alloc_data, &mut free_alloc);
                // no space remains
                if free_alloc.space_size() != 0 {
                    self.bucket_list.add_to_list(&mut free_alloc);
                } else {
                    // Edge Case: If the remaining space is too small to be used again,
                    // simply return a larger block
                    code_block::set_free(alloc_data.data_start(), false);
                    copy_code_block_to_end(alloc_data);
                }
            }
            self.check_integrity();
            self.check_alloc(alloc_data);
            alloc_data.check_space();
            alloc_data.check_consistency();
        }
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
    pub fn delete_block(&mut self, alloc_data: &mut AllocationData) {
        alloc_data.set_page(self);
        self.check_integrity();
        let (memory_block_size, code_block_start) =
            unsafe { code_block::read_from_right(alloc_data.data_start().offset(-1)) };
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
        left_neighbor.set_page(alloc_data.page());
        let mut right_neighbor = AllocationData::new();
        right_neighbor.set_page(alloc_data.page());
        right_neighbor.set_data_start(unsafe {
            code_block_start.add(2 * code_block_size + memory_block_size) as *mut u8
        });
        if right_neighbor.data_start() as usize > self.end_of_page as usize {
            panic!("Mara: Page Overflow");
        }
        if self.start_of_page < code_block_start {
            left_neighbor.set_data_start(get_left_neighbor(alloc_data) as *mut u8);
        }
        if !left_neighbor.data_start().is_null() && !code_block::is_free(left_neighbor.data_start())
        {
            left_neighbor.set_data_start(core::ptr::null_mut());
        }
        if !right_neighbor.data_start().is_null()
            && (right_neighbor.data_start() as usize >= self.end_of_page as usize
                || !code_block::is_free(right_neighbor.data_start()))
        {
            right_neighbor.set_data_start(core::ptr::null_mut());
        }
        unsafe { self.merge_free_space(&mut left_neighbor, alloc_data, &mut right_neighbor) };
        #[cfg(feature = "consistency-checks")]
        {
            unsafe {
                assert!(
                    (left_neighbor.data_start().is_null()
                        && self.bucket_list.is_in_list(alloc_data).0
                        && code_block::is_free(code_block_start))
                        // || (self.bucket_list.is_in_list(left_neighbor).0
                            // && code_block::is_free(left_neighbor)),
                )
            };
        }
        self.check_integrity();
    }
    /// #### return
    /// a pointer to the first byte in the page
    pub fn start_of_page(&self) -> *const u8 {
        self.start_of_page
    }
    /// #### return
    /// a pointer to the last byte in the page
    pub fn end_of_page(&self) -> *const u8 {
        self.end_of_page
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
    ) {
        alloc_data.check_consistency();
        #[cfg(feature = "consistency-checks")]
        {
            assert!(r_alloc.data_start().is_null() || self.bucket_list.is_in_list(r_alloc).0);
            assert!(l_alloc.data_start().is_null() || self.bucket_list.is_in_list(l_alloc).0);
        }
        if l_alloc.data_start().is_null() {
            if !r_alloc.data_start().is_null() {
                self.bucket_list.delete_from_list(r_alloc);
                self.merge_with_right(alloc_data, r_alloc);
            }
            code_block::set_free(alloc_data.data_start(), true);
            alloc_data.set_code_block_size(code_block::get_block_size(alloc_data.data_start()));
            copy_code_block_to_end(alloc_data);
            self.bucket_list.add_to_list(alloc_data);
            alloc_data.check_consistency();
            #[cfg(feature = "consistency-checks")]
            {
                assert!(self.bucket_list.is_in_list(alloc_data).0);
            }
        } else {
            if !r_alloc.data_start().is_null() {
                self.bucket_list.delete_from_list(r_alloc);
                self.merge_with_right(alloc_data, r_alloc);
            }
            self.bucket_list.delete_from_list(l_alloc);

            self.merge_with_left(l_alloc.data_start(), alloc_data);
            code_block::set_free(l_alloc.data_start(), true);
            l_alloc.set_code_block_size(code_block::get_block_size(l_alloc.data_start()));
            copy_code_block_to_end(l_alloc);
            self.bucket_list.add_to_list(l_alloc);
            alloc_data.check_consistency();
            #[cfg(feature = "consistency-checks")]
            {
                assert!(self.bucket_list.is_in_list(l_alloc).0);
            }
        }
    }
    /// Merges both blocks to one. The types of Blocks are ignored.
    #[inline]
    unsafe fn merge_with_left(&self, left_block: *mut u8, alloc_data: &mut AllocationData) {
        #[cfg(feature = "consistency-checks")]
        {
            assert!(code_block::is_free(left_block));
        }
        alloc_data.set_data_start(left_block);
        //let right_end = get_right_most_end(middle_block);
        let code_block_size = code_block::generate_code_block_for_internal_size(
            left_block,
            alloc_data.calculate_data_size() as usize + 1,
            true,
        );
        alloc_data.set_code_block_size(code_block_size);
        copy_code_block_to_end(alloc_data);
        #[cfg(feature = "consistency-checks")]
        {
            assert!(code_block::is_free(left_block));
            assert!(
                code_block::read_from_left(left_block)
                    == alloc_data.data_end() as usize - left_block as usize - 2 * code_block_size
                        + 1
            );
        }
    }
    //// Merges both blocks to one. The types of Blocks are ignored.
    #[inline]
    unsafe fn merge_with_right(
        &self,
        alloc_data: &mut AllocationData,
        r_alloc: &mut AllocationData,
    ) {
        #[cfg(feature = "consistency-checks")]
        {
            assert!(code_block::is_free(r_alloc.data_start()));
        }
        alloc_data.set_data_end(r_alloc.data_end());
        let code_block_size = code_block::generate_code_block_for_internal_size(
            alloc_data.data_start(),
            alloc_data.calculate_data_size() as usize + 1,
            true,
        );
        alloc_data.set_space_size(code_block_size);
        copy_code_block_to_end(alloc_data);
        #[cfg(feature = "consistency-checks")]
        {
            assert!(code_block::is_free(alloc_data.data_start()));
            assert!(
                code_block::read_from_left(alloc_data.data_start())
                    == alloc_data.calculate_data_size() - 2 * code_block_size + 1
            );
        }
    }
    /// Splits ``free_alloc`` into two separate parts.
    /// ``alloc data`` will be the left side of the split,
    /// and ``free_alloc`` will be the right side.
    /// The left part is expected to be used as allocation **return**,
    /// so its ``next`` pointer will not be updated.
    /// Instead the old next pointer will be the new next pointer of right space.
    /// If the remaining space is not big enough it will not be split.
    /// In that case an AllocationData with a ``space_size`` of ``0`` is returned
    /// During the process the code blocks are update accordingly
    #[inline]
    fn split_free_space(&self, alloc_data: &mut AllocationData, free_alloc: &mut AllocationData) {
        unsafe {
            self.check_split_pre(alloc_data, free_alloc);

            // Space to small to cut something of
            if free_alloc.space_size().sub(alloc_data.space_size()) < SMALLEST_POSSIBLE_FREE_SPACE {
                free_alloc.set_space_size(0);
            }
            // space is big enough to cut
            else {
                // alloc_data.set_space(alloc_data.data_start().add(alloc_data.code_block_size()));
                // alloc_data.set_data_end(alloc_data.space().add(alloc_data.space_size()));
                // code blocks might shrink here
                write_space_size_code_blocks(alloc_data, false);
                free_alloc.set_page(alloc_data.page());
                free_alloc.set_data_start(alloc_data.data_end().add(1));
                write_data_size_code_blocks(free_alloc, true);

                self.check_split_post(alloc_data, &free_alloc);
            }
        }
    }
    /// Check that page start is before its end
    #[inline]
    fn check_integrity(&self) {
        #[cfg(feature = "consistency-checks")]
        {
            if self.start_of_page as usize > self.end_of_page as usize {
                dbg!(self.start_of_page);
                dbg!(self.end_of_page);
                panic!("start of page is after end of page")
            }
        }
    }
    /// check that alloc pointers are in page boundaries
    #[inline]
    fn check_alloc(&self, alloc_data: &AllocationData) {
        #[cfg(feature = "consistency-checks")]
        {
            self.check_alloc_start(alloc_data);
            self.check_alloc_end(alloc_data);
            self.check_alloc_space(alloc_data);
        }
    }
    /// check that alloc.data_start is in page boundaries
    #[inline]
    pub fn check_alloc_start(&self, alloc_data: &AllocationData) {
        if (alloc_data.data_start() as usize) < self.start_of_page as usize {
            dbg!(self.start_of_page);
            dbg!(alloc_data.data_start());
            panic!("Allocation start is left of page start")
        }
        if alloc_data.data_start() as usize > self.end_of_page as usize {
            dbg!(self.end_of_page);
            dbg!(alloc_data.data_start());
            panic!("Allocation start is right of page end")
        }
    }
    /// check that alloc.data_end is in page boundaries
    #[inline]
    pub fn check_alloc_end(&self, alloc_data: &AllocationData) {
        if alloc_data.data_end() as usize > self.end_of_page as usize {
            dbg!(self.end_of_page);
            dbg!(alloc_data.data_end());
            panic!("Allocation end is right of page end")
        }
        if (alloc_data.data_end() as usize) < self.start_of_page as usize {
            dbg!(self.start_of_page);
            dbg!(alloc_data.data_end());
            panic!("Allocation end is left of page start")
        }
    }
    /// check that alloc.space pointer is in page boundaries
    #[inline]
    pub fn check_alloc_space(&self, alloc_data: &AllocationData) {
        #[cfg(feature = "consistency-checks")]
        {
            unsafe {
                if !(alloc_data.space() as usize > self.start_of_page as usize
                    && (alloc_data.space().add(alloc_data.space_size()) as usize)
                        < self.end_of_page as usize)
                {
                    dbg!(alloc_data.space());
                    dbg!(alloc_data.space().add(alloc_data.space_size()));
                    panic!("allocation space pointer is outside of page boundaries")
                }
            }
        }
    }
    // check preconditions of split
    #[inline]
    pub fn check_split_pre(&self, left_alloc: &AllocationData, right_alloc: &AllocationData) {
        #[cfg(feature = "consistency-checks")]
        {
            // pointer is in page
            self.check_alloc_start(left_alloc);
            // right alloc is big enough for left alloc to fit in
            // and small enough to fit in page
            right_alloc.check_space_size(
                left_alloc.space_size(),
                self.page_size()
                    .sub(2 * code_block::get_needed_code_block_size(right_alloc.space_size())),
            )
        }
    }
    // check boundaries and code blocks and cache after a successful split
    #[inline]
    pub fn check_split_post(&self, left_alloc: &AllocationData, right_alloc: &AllocationData) {
        #[cfg(feature = "consistency-checks")]
        {
            unsafe {
                // read code blocks
                let left_memory_size = code_block::read_from_left(left_alloc.data_start());
                let right_memory_size = code_block::read_from_left(right_alloc.data_start());
                let left_code_block_size = code_block::get_block_size(left_alloc.data_start());
                // let right_code_block_size = code_block::get_block_size(right_alloc.data_start());
                right_alloc.check_next_boundaries();
                self.check_alloc(left_alloc);
                self.check_alloc(right_alloc);
                right_alloc.check_space_size(
                    size_of::<NextPointerType>(),
                    self.page_size()
                        .sub(left_alloc.calculate_data_size())
                        .sub(2 * right_alloc.code_block_size()),
                );
                // allocs are direct neighbors
                if left_alloc
                    .data_start()
                    .add(left_memory_size)
                    .add(2 * left_code_block_size)
                    != right_alloc.data_start()
                {
                    dbg!(left_alloc
                        .data_start()
                        .add(left_memory_size)
                        .add(2 * left_code_block_size));
                    dbg!(right_alloc.data_start());
                    panic!("split allocs are not direct neighbors")
                }
                // correct free bits
                if code_block::is_free(left_alloc.data_start()) {
                    dbg!(code_block::is_free(left_alloc.data_start()));
                    panic!("free bit not set after split");
                }
                if !code_block::is_free(right_alloc.data_start()) {
                    dbg!(!code_block::is_free(right_alloc.data_start()));
                    panic!("free bit not set after split");
                }
                // correct free bits on the right side
                if code_block::is_free(left_alloc.calculate_right_code_block()) {
                    dbg!(code_block::is_free(left_alloc.calculate_right_code_block()));
                    panic!("free bit not set after split");
                }
                if !code_block::is_free(right_alloc.calculate_right_code_block()) {
                    dbg!(!code_block::is_free(
                        right_alloc.calculate_right_code_block()
                    ));
                    panic!("free bit not set after split");
                }
                // equal code block data
                if left_memory_size
                    != code_block::read_from_left(left_alloc.calculate_right_code_block())
                {
                    dbg!(left_memory_size);
                    dbg!(code_block::read_from_left(
                        left_alloc.calculate_right_code_block()
                    ));
                    panic!("code blocks are not equal")
                }
                if right_memory_size
                    != code_block::read_from_left(right_alloc.calculate_right_code_block())
                {
                    dbg!(right_memory_size);
                    dbg!(code_block::read_from_left(
                        right_alloc.calculate_right_code_block()
                    ));
                    panic!("code blocks are not equal")
                }
                // cached space matches with code block space
                left_alloc.check_space_size(left_memory_size, left_memory_size);
                right_alloc.check_space_size(right_memory_size, right_memory_size);
            }
        }
    }
}
