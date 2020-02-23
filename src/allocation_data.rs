use crate::code_block;
use crate::globals::*;
use crate::space::*;
use crate::Page;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct AllocationData {
    /// The first byte of the allocation.
    /// This is also the start of the left code block.
    data_start: Option<*mut u8>,
    /// The last byte of the allocation.
    /// This is also the end of the right code block.
    data_end: Option<*mut u8>,
    /// The amount of bytes a code block needs to encode the space size.
    code_block_size: Option<usize>,
    /// Marks start of return data and makes functions for free
    /// space administration accessible
    pub space: Space,
    // A pointer to the page in which this allocation resides
    page: Option<*mut Page>,
}
impl AllocationData {
    pub fn new() -> Self {
        Self {
            data_start: None,
            data_end: None,
            code_block_size: None,
            space: Space::new(),
            page: None,
        }
    }
    ///////////////////////////////////////////////////
    //Getter
    pub fn data_start(&self) -> *mut u8 {
        self.data_start.expect("Uninitialized data start pointer")
    }
    pub fn data_end(&self) -> *mut u8 {
        self.data_end.expect("Uninitialized data end pointer")
    }
    pub fn code_block_size(&self) -> usize {
        self.code_block_size.expect("Uninitialized code block size")
    }
    pub fn page(&self) -> *mut Page {
        self.page.expect("Uninitialized page pointer")
    }
    //////////////////////////////////////////////////////////
    // Generated data
    #[inline]
    pub fn calculate_data_size(&self) -> usize {
        if let Some(data_start) = self.data_start {
            if let Some(data_end) = self.data_end {
                return data_end as usize - data_start as usize + 1;
            }
            let code_block_size = match self.code_block_size {
                Some(size) => size,
                None => code_block::read_from_left(data_start),
            };
            let space_size = match self.space.is_some() {
                true => self.space.size(),
                false => code_block::read_from_left(data_start),
            };
            return 2 * code_block_size + space_size;
        }
        panic!("Cannot determine data size")
    }
    #[inline]
    pub fn calculate_start_of_page(&self) -> *const u8 {
        unsafe { (*self.page()).start_of_page() }
    }
    /// calculates the first byte of the right code block
    /// ignores the currently cached value
    #[inline]
    pub fn calculate_right_code_block(&self) -> *mut u8 {
        unsafe {
            if self.space.is_some() {
                let space_size = match self.space.size_is_some() {
                    true => self.space.size(),
                    false => code_block::read_from_right(self.space.ptr().sub(1)).0,
                };
                self.space.ptr().add(space_size)
            }
            // expect data_start if space pointer is not set
            else {
                let space_size = match self.space.size_is_some() {
                    true => self.space.size(),
                    false => code_block::read_from_left(self.data_start()),
                };
                let code_block_size = match self.code_block_size {
                    Some(size) => size,
                    None => code_block::get_needed_code_block_size(space_size),
                };
                self.data_start().add(space_size).add(code_block_size)
            }
        }
    }
    //////////////////////////////////////////////////////////////
    // setter
    pub fn set_data_start(&mut self, data: *mut u8) {
        self.data_start = Some(data)
    }
    pub fn set_data_end(&mut self, data: *mut u8) {
        self.data_end = Some(data)
    }
    pub fn set_code_block_size(&mut self, data: usize) {
        self.code_block_size = Some(data)
    }
    pub fn set_page(&mut self, data: *mut Page) {
        self.page = Some(data)
    }
    ///////////////////////////////////////////////////////
    // data manipulation

    /// Reads data from code blocks and updates the cached pointers.
    /// Tries to read from ``data_start``, ``space.ptr-1`` and ``data_end``
    /// in that order.
    pub fn read_and_cache_code_blocks(&mut self) {
        unsafe {
            // first try from data start
            if let Some(start) = self.data_start {
                self.space.set_size(code_block::read_from_left(start));
                self.set_code_block_size(code_block::get_block_size(start));
                self.space.set_ptr(start.add(self.code_block_size()));
                self.set_data_end(start.add(2 * self.code_block_size()).add(self.space.size()));
            } else {
                //then from space start
                if self.space.is_some() {
                    let (memory_size, block) = code_block::read_from_right(self.space.ptr());
                    self.space.set_size(memory_size);
                    self.set_code_block_size(code_block::get_block_size(block));
                    self.set_data_start(block);
                    self.set_data_end(
                        self.data_start()
                            .add(2 * self.code_block_size())
                            .add(self.space.size()),
                    );
                }
                // end lastly try the end pointer or panic
                else {
                    let (memory_size, block) = code_block::read_from_right(self.data_end());
                    self.space.set_size(memory_size);
                    self.set_code_block_size(code_block::get_block_size(block));
                    self.set_data_start(block.sub(self.code_block_size()).sub(memory_size));
                    self.space
                        .set_ptr(self.data_start().add(self.code_block_size()));
                }
            }
            self.check_consistency();
        }
    }
    /// Returns the allocation that succeeds self or None if self is the 
    /// last in the page.
    /// Caches the information that is stored in the code blocks
    pub fn right_neighbor(&self) -> Option<AllocationData> {
        unsafe {
            let start = self.data_end().add(1);
            if start < (*self.page()).end_of_page() as *mut u8 {
                let mut right = AllocationData::new();
                right.set_page(self.page());
                right.set_data_start(start);
                right.read_and_cache_code_blocks();
                Some(right)
            } else {
                None
            }
        }
    }
    /// Returns the allocation that precedes self or None if self is the 
    /// first in the page.
    /// Caches the information that is stored in the code blocks
    pub fn left_neighbor(&self) -> Option<AllocationData> {
        unsafe {
            let end = self.data_start().sub(1);
            if end < (*self.page()).start_of_page() as *mut u8 {
                let mut left = AllocationData::new();
                left.set_page(self.page());
                left.set_data_end(end);
                left.read_and_cache_code_blocks();
                Some(left)
            } else {
                None
            }
        }
    }

    /// Copies a code block from the beginning of space to the end of space
    pub unsafe fn copy_code_block_to_end(&mut self) {
        #[cfg(feature = "consistency-checks")]
        {
            assert!(self.code_block_size() > 0);
        }
        let mut current_position: *mut u8 =
            ((self.data_end() as usize - self.code_block_size()) + 1) as *mut u8;
        for i in 0..self.code_block_size() {
            if current_position as usize <= self.data_end() as usize {
                *current_position = *(self.data_start().offset(i as isize));
            } else {
                return;
            }
            current_position = current_position.offset(1);
        }
        #[cfg(feature = "consistency-checks")]
        {
            assert!(current_position.offset(-1) == self.data_end());
            let (right_block_size, _) = code_block::read_from_right(self.data_end());
            assert!(code_block::read_from_left(self.data_start()) == right_block_size,);
        }
    }
    /// Write a code block that is consistent with the allocation size (``data_start``
    /// to ``data_end``).
    /// The code block is copied to the end of the allocation and the free bit is
    /// determined by ``is_free``.
    /// Allocation cache for ``space`` and ``space size is updated``
    /// The ``next`` pointer is also written at the correct location
    pub unsafe fn write_data_size_code_blocks(&mut self, is_free: bool) {
        let code_block_size = code_block::generate_code_block_for_internal_size(
            self.data_start(),
            self.calculate_data_size(),
            is_free,
        );
        self.set_code_block_size(code_block_size);
        self.copy_code_block_to_end();
        // update allocation
        self.space
            .set_size(self.calculate_data_size() - 2 * code_block_size);
        self.space.set_ptr(self.data_start().add(code_block_size));
        self.space.write_next(self.calculate_start_of_page());
        #[cfg(feature = "consistency-checks")]
        {
            let (right_block_size, _) = code_block::read_from_right(self.data_end());
            assert!(code_block::read_from_left(self.data_start()) == right_block_size,);
        }
    }
    /// Write a code block that is consistent with the ``space_size`` of an allocation.
    /// The code block is copied to the end of the allocation and the free bit is
    /// determined by ``is_free``.
    /// Allocation cache for ``data_end`` and ``space`` is updated (allocation might
    /// shrink if the code block get smaller)
    pub unsafe fn write_space_size_code_blocks(&mut self, is_free: bool) {
        code_block::generate_code_block_for_payload_size(self, is_free);
        self.space
            .set_ptr(self.data_start().add(self.code_block_size()));
        self.set_data_end(
            self.space
                .ptr()
                .add(self.space.size())
                .add(self.code_block_size())
                .sub(1),
        );
        self.copy_code_block_to_end();
        #[cfg(feature = "consistency-checks")]
        {
            let (right_block_size, _) = code_block::read_from_right(self.data_end());
            assert!(code_block::read_from_left(self.data_start()) == right_block_size,);
        }
    }
    //////////////////////////////////////////////////////////
    // Consistency checks
    #[inline]
    pub fn check_space_size(&self, min: usize, max: usize) {
        #[cfg(feature = "consistency-checks")]
        {
            if self.space.size() < min {
                dbg!(self.space.size());
                dbg!(min);
                panic!("Space is smaller as expected");
            }
            if self.space.size() > max {
                dbg!(self.space.size());
                dbg!(max);
                panic!("space is larger as expected");
            }
        }
    }
    #[inline]
    pub fn check_data_size(&self, min: usize, max: usize) {
        #[cfg(feature = "consistency-checks")]
        {
            if self.calculate_data_size() < min {
                dbg!(self.calculate_data_size());
                dbg!(min);
                panic!("Space size is smaller as expected");
            }
            if self.calculate_data_size() > max {
                dbg!(self.calculate_data_size());
                dbg!(max);
                panic!("Space size is larger as expected");
            }
        }
    }
    #[inline]
    pub fn check_space(&self) {
        #[cfg(feature = "consistency-checks")]
        {
            if self.space.ptr().is_null() {
                dbg!(self.space.ptr());
                panic!("space is null")
            }
        }
    }
    #[inline]
    pub fn check_left_free(&self, expected: bool) {
        #[cfg(feature = "consistency-checks")]
        {
            //if we know the data start check the block
            if self.data_start.is_some() {
                if code_block::is_free(self.data_start()) != expected {
                    dbg!(code_block::is_free(self.data_start()));
                    dbg!(expected);
                    panic!("Free bit was not set correctly")
                }
            }
            // else expect to know the space pointer and read from right
            else {
                unsafe {
                    let block = code_block::read_from_right(self.space.ptr().sub(1)).1;
                    if code_block::is_free(block) != expected {
                        dbg!(code_block::is_free(block));
                        dbg!(expected);
                        panic!("Free bit was not set correctly")
                    }
                }
            }
        }
    }
    #[inline]
    pub fn check_consistency(&self) {
        #[cfg(feature = "consistency-checks")]
        {
            unsafe {
                // let left_size
                // let right_size
                // let left_next = free_space::set_left_next(self);
                // let right_next = free_space::set_right_next(self);

                // check data size
                if self.data_start() as usize >= self.data_end() as usize {
                    dbg!(self.data_start());
                    dbg!(self.data_end());
                    panic!("data start and end are crossed")
                }
                // check space boundaries
                if (self.space.ptr() as usize) >= self.data_end() as usize
                    || (self.data_start() as usize) > self.space.ptr() as usize
                {
                    dbg!(self.data_start());
                    dbg!(self.space.ptr());
                    dbg!(self.data_end());
                    panic!("space is outside of data")
                }
                // check space position
                if self.space.ptr()
                    != self
                        .data_start()
                        .add(code_block::get_block_size(self.data_start()))
                {
                    dbg!(self.space.ptr());
                    dbg!(self.data_start().add(self.code_block_size()));
                    panic!("space pointer is not at expected position");
                }
                // check both free bits are consistent
                let left_free = code_block::is_free(self.data_start());
                let right_free = code_block::is_free(self.calculate_right_code_block());
                if left_free != right_free {
                    dbg!(left_free);
                    dbg!(right_free);
                    panic!("free flags inconsistent")
                }
                // check both codeblocks encode the same size
                let left_size = code_block::read_from_left(self.data_start());
                let right_size = code_block::read_from_left(self.calculate_right_code_block());
                if left_size != right_size {
                    dbg!(left_size);
                    dbg!(right_size);
                    panic!("Code blocks encode different data");
                }
            }
        }
    }
    #[inline]
    pub fn check_next_boundaries(&self) {
        #[cfg(feature = "consistency-checks")]
        {
            unsafe {
                let next_target = self.space.next() as *const u8;
                let start_of_page = (*self.page()).start_of_page();
                let end_of_page = (*self.page()).end_of_page();
                if !next_target.is_null()
                    && (next_target <= start_of_page || next_target >= end_of_page)
                {
                    dbg!(next_target);
                    dbg!(start_of_page);
                    dbg!(end_of_page);
                    panic!("next points outside of the page")
                }
            }
        }
    }
}
