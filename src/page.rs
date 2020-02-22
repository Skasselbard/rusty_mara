use crate::bucket_list::BucketList;
use crate::code_block;
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
            alloc_data
                .space
                .set_ptr(alloc_data.data_start().add(alloc_data.code_block_size()));
            alloc_data
                .space
                .set_size(page_size - 2 * alloc_data.code_block_size());
            self.bucket_list.insert(&mut alloc_data);

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
        alloc_data.copy_code_block_to_end();
        alloc_data.space.set_next(core::ptr::null_mut());
    }
    /// tries to reserve a dynamic block in this page, and returns it
    pub fn get_dynamic_block(&mut self, alloc_data: &mut AllocationData) {
        unsafe {
            alloc_data.set_page(self);
            alloc_data.check_space_size(1, self.page_size());
            self.check_integrity();

            let mut free_alloc = &mut alloc_data.clone();
            self.bucket_list.get_free_space(&mut free_alloc);
            if free_alloc.space.ptr().is_null() {
                self.check_integrity();
                return;
            } else {
                // Remove this free space from list
                // the remaining space will be added again later
                self.bucket_list.remove(free_alloc);
                // Calculate where the allocation starts
                // It will be at the beginning of the found free space
                alloc_data.set_data_start(free_alloc.space.ptr().sub(
                    code_block::get_needed_code_block_size(free_alloc.space.size()),
                ));
                // next has to be cached
                free_alloc
                    .space
                    .set_next(free_alloc.space.read_next(self.start_of_page));
                // cache the end of the free space for later
                free_alloc.set_data_end(
                    free_alloc
                        .space
                        .ptr()
                        .add(free_alloc.space.size())
                        .add(code_block::get_block_size(alloc_data.data_start()))
                        .sub(1),
                );
                // split the free space in two
                self.split_free_space(alloc_data, free_alloc);
                // no space remains
                if free_alloc.space.size() != 0 {
                    self.bucket_list.insert(free_alloc);
                } else {
                    // Edge Case: If the remaining space is too small to be used again,
                    // simply return a larger block
                    code_block::set_free(alloc_data.data_start(), false);
                    alloc_data.copy_code_block_to_end();
                }
            }
            self.check_integrity();
            alloc_data.check_space();
            self.check_dynamic_new_post(alloc_data);
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
            if free_alloc.space.size().sub(alloc_data.space.size()) < SMALLEST_POSSIBLE_FREE_SPACE {
                free_alloc.space.set_size(0);
            }
            // space is big enough to cut
            else {
                // alloc_data.space.set_ptr(alloc_data.data_start().add(alloc_data.code_block_size()));
                // alloc_data.set_data_end(alloc_data.space.ptr().add(alloc_data.space.size()));
                // code blocks might shrink here
                alloc_data.write_space_size_code_blocks(false);
                free_alloc.set_page(alloc_data.page());
                free_alloc.set_data_start(alloc_data.data_end().add(1));
                free_alloc.write_data_size_code_blocks(true);

                self.check_split_post(alloc_data, &free_alloc);
            }
        }
    }
    /// Deletes a reserved block and adds it into bucket list again.
    /// If the neighboring spaces are free they are merged wit this space.
    pub fn delete_block(&mut self, alloc_data: &mut AllocationData) {
        unsafe {
            alloc_data.set_page(self);
            self.check_integrity();
            let (memory_block_size, code_block_start) =
                code_block::read_from_right(alloc_data.space.ptr().offset(-1));
            alloc_data.set_data_start(code_block_start as *mut u8);
            alloc_data.space.set_size(memory_block_size);
            alloc_data.set_code_block_size(code_block::get_block_size(code_block_start));
            #[cfg(feature = "statistic")]
            {
                Statistic::freeDynamic(memory_block_size, first_byte);
            }
            alloc_data.set_data_end(
                alloc_data
                    .space
                    .ptr()
                    .add(alloc_data.space.size())
                    .add(alloc_data.code_block_size())
                    .sub(1),
            );
            self.merge_with_neighbors(alloc_data);
            self.check_integrity();
        }
    }
    /// checks both neighboring spaces if they are free
    /// if so they are merged with the given allocation
    #[inline]
    fn merge_with_neighbors(&mut self, alloc_data: &mut AllocationData) {
        unsafe {
            let mut left_alloc = AllocationData::new();
            left_alloc.set_page(self);
            left_alloc.set_data_end(alloc_data.data_start().sub(1));
            let (left_space_size, l_code_block_start) =
                code_block::read_from_right(left_alloc.data_end());
            // check if we can merge left and respect the page boundaries
            if self.start_of_page() < left_alloc.data_end()
                && code_block::is_free(l_code_block_start)
            {
                left_alloc.space.set_size(left_space_size);
                left_alloc.set_code_block_size(code_block::get_block_size(l_code_block_start));
                left_alloc.set_data_start(
                    l_code_block_start
                        .sub(left_alloc.space.size())
                        .sub(left_alloc.code_block_size()),
                );
                alloc_data.set_data_start(left_alloc.data_start());
                self.bucket_list.remove(&mut left_alloc);
                self.check_alloc_start(&left_alloc);
                self.bucket_list.check_in_list(&left_alloc, false);
            }

            let mut right_alloc = AllocationData::new();
            right_alloc.set_data_start(alloc_data.data_end().add(1));
            right_alloc.set_page(self);
            // check if we can merge right and respact the page boundaries
            if self.end_of_page > right_alloc.data_start()
                && code_block::is_free(right_alloc.data_start())
            {
                right_alloc
                    .space
                    .set_size(code_block::read_from_left(right_alloc.data_start()));
                right_alloc
                    .set_code_block_size(code_block::get_block_size(right_alloc.data_start()));
                alloc_data.set_data_end(
                    alloc_data
                        .data_end()
                        .add(right_alloc.space.size())
                        .add(2 * right_alloc.code_block_size()),
                );
                self.bucket_list.remove(&mut right_alloc);
                self.check_alloc_end(&right_alloc);
                self.bucket_list.check_in_list(&right_alloc, false);
            }
            // write code blocks with set free flag
            // and get code block and space information for free
            alloc_data.write_data_size_code_blocks(true);
            self.bucket_list.insert(alloc_data);
            self.bucket_list.check_in_list(alloc_data, true);
        }
    }
    #[inline]
    pub fn page_size(&self) -> usize {
        self.end_of_page as usize - self.start_of_page as usize
    }
    #[inline]
    /// #### return the next page in the ring storage
    pub fn get_next_page(&self) -> *mut Self {
        self.next_page
    }
    /// sets the next page
    /// #### next_page
    /// the next page
    #[inline]
    pub fn set_next_page(&mut self, next_page: *mut Self) {
        if next_page != core::ptr::null_mut() {}
        self.next_page = next_page;
    }
    /// #### first_byte
    /// a pointer to the block of interest
    /// #### return
    /// true if the pointer is in between the start of page and the left most byte of the static sector.
    /// false otherwise. Blocks in the static sector CANNOT be detected with this function.
    #[inline]
    pub fn block_is_in_space(&self, first_byte: *const u8) -> bool {
        self.start_of_page <= first_byte && first_byte < self.end_of_page
    }
    /// #### return
    /// a pointer to the first byte in the page
    #[inline]
    pub fn start_of_page(&self) -> *const u8 {
        self.start_of_page
    }
    /// #### return
    /// a pointer to the last byte in the page
    #[inline]
    pub fn end_of_page(&self) -> *const u8 {
        self.end_of_page
    }
    /// #### return
    /// the bucket list
    #[inline]
    pub fn bucket_list(&self) -> &BucketList {
        &self.bucket_list
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
                if !(alloc_data.space.ptr() as usize > self.start_of_page as usize
                    && (alloc_data.space.ptr().add(alloc_data.space.size()) as usize)
                        < self.end_of_page as usize)
                {
                    dbg!(alloc_data.space.ptr());
                    dbg!(alloc_data.space.ptr().add(alloc_data.space.size()));
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
                left_alloc.space.size(),
                self.page_size()
                    .sub(2 * code_block::get_needed_code_block_size(right_alloc.space.size())),
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
    fn check_dynamic_new_post(&self, alloc: &AllocationData) {
        #[cfg(feature = "consistency-checks")]
        {
            unsafe {
                self.check_alloc_start(alloc);
                self.check_alloc_end(alloc);
                alloc.check_consistency();
                // check consistency of left neighbor
                if alloc.data_start() > self.start_of_page as *mut u8 {
                    let left_alloc = &mut AllocationData::new();
                    left_alloc.set_data_end(alloc.data_start().sub(1));
                    let (memory_size, block) = code_block::read_from_right(left_alloc.data_end());
                    left_alloc.space.set_size(memory_size);
                    left_alloc.set_code_block_size(code_block::get_block_size(block));
                    left_alloc
                        .set_data_start(block.sub(left_alloc.code_block_size()).sub(memory_size));
                    left_alloc
                        .space
                        .set_ptr(left_alloc.data_start().add(left_alloc.code_block_size()));
                    left_alloc.check_consistency();
                }
                // check consistency of right neighbor
                if alloc.data_start() > self.start_of_page as *mut u8 {
                    let right_alloc = &mut AllocationData::new();
                    right_alloc.set_data_start(alloc.data_end().add(1));
                    right_alloc
                        .space
                        .set_size(code_block::read_from_left(right_alloc.data_start()));
                    right_alloc
                        .set_code_block_size(code_block::get_block_size(right_alloc.data_start()));
                    right_alloc.set_data_end(
                        right_alloc
                            .data_start()
                            .add(2 * right_alloc.code_block_size())
                            .add(right_alloc.space.size()),
                    );
                    right_alloc
                        .space
                        .set_ptr(right_alloc.data_start().add(right_alloc.code_block_size()));
                    right_alloc.check_consistency();
                }
            }
        }
    }
}
