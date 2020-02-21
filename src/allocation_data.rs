use crate::code_block;
use crate::free_space;
use crate::globals::*;
use crate::Page;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct AllocationData {
    /// The first byte of the allocation.
    /// This is also the start of the left code block.
    data_start: Option<*mut u8>,
    /// The last byte of the allocation.
    /// This is also the end of the right code block.
    data_end: Option<*mut u8>,
    /// The start of the right code block.
    /// The right code block is located at the right most and of the allocation.
    code_block_right: Option<*mut u8>,
    /// The amount of bytes a code block needs to encode the space size.
    code_block_size: Option<usize>,
    /// The beginning of a space.
    /// This pointer will be returnded by mara.
    /// Succeeds the left code block.
    /// Precedes the right code block
    /// If this is a free space, this location is used to store the next pointer.
    space: Option<*mut u8>,
    /// Size of the space in bytes.
    /// Depending on allocation or deallocation this is an intended size.
    /// or an actual size.
    /// The right code block is located at space + space_size.
    space_size: Option<usize>,
    /// Marks if this space is currently free space or occupied.
    /// Occupied space was previously allocated and must not be mutated.
    /// Free space can be used for a future allocation.
    // space_is_free: Option<bool>,
    /// A pointer to the next free space.
    /// Succeeds the left code block.
    /// Points to another next pointer (NOT the data start of another allocation).
    /// Null if there is no successor.
    next_pointer: Option<*mut NextPointerType>,
    // A pointer to the page in which this allocation resides
    page: Option<*mut Page>,
}
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum AllocDataType {
    Start,
    End,
    BlockRight,
    BlockSize,
    Space,
    SpaceSize,
    SpaceFree,
    NextPtr,
    Page,
}
impl AllocationData {
    pub fn new() -> Self {
        Self {
            data_start: None,
            data_end: None,
            code_block_right: None,
            code_block_size: None,
            space: None,
            space_size: None,
            next_pointer: None,
            page: None,
        }
    }

    pub fn space_is_init(&self) -> bool {
        match self.space {
            Some(_) => true,
            None => false,
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
    pub fn code_block_right(&self) -> *mut u8 {
        self.code_block_right
            .expect("Uninitialized right code block pointer")
    }
    pub fn code_block_size(&self) -> usize {
        self.code_block_size.expect("Uninitialized code block size")
    }
    pub fn space(&self) -> *mut u8 {
        self.space.expect("Uninitialized space pointer")
    }
    pub fn space_size(&self) -> usize {
        self.space_size.expect("Uninitialized space size")
    }
    pub fn next_pointer(&self) -> *mut NextPointerType {
        self.next_pointer.expect("Uninitialized next pointer")
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
                return data_end as usize - data_start as usize +1;
            }
            unsafe {
                let code_block_size = match self.code_block_size {
                    Some(size) => size,
                    None => code_block::read_from_left(data_start),
                };
                let space_size = match self.space_size {
                    Some(size) => size,
                    None => code_block::read_from_left(data_start),
                };
                return 2 * code_block_size + space_size;
            }
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
            if let Some(space) = self.space {
                let space_size = match self.space_size {
                    Some(size) => size,
                    None => code_block::read_from_right(space.sub(1)).0,
                };
                space.add(space_size)
            }
            // expect data_start if space pointer is not set
            else {
                let space_size = match self.space_size {
                    Some(size) => size,
                    None => code_block::read_from_left(self.data_start()),
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
    pub fn set_code_block_right(&mut self, data: *mut u8) {
        self.code_block_right = Some(data)
    }
    pub fn set_code_block_size(&mut self, data: usize) {
        self.code_block_size = Some(data)
    }
    pub fn set_space(&mut self, data: *mut u8) {
        self.space = Some(data)
    }
    pub fn set_space_size(&mut self, data: usize) {
        self.space_size = Some(data)
    }
    pub fn set_next_pointer(&mut self, data: *mut NextPointerType) {
        self.next_pointer = Some(data)
    }
    pub fn set_page(&mut self, data: *mut Page) {
        self.page = Some(data)
    }
    //////////////////////////////////////////////////////////
    // Consistency checks
    #[inline]
    pub fn check_space_size(&self, min: usize, max: usize) {
        #[cfg(feature = "consistency-checks")]
        {
            if self.space_size() < min {
                dbg!(self.space_size());
                dbg!(min);
                panic!("Space is smaller as expected");
            }
            if self.space_size() > max {
                dbg!(self.space_size());
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
            if self.space().is_null() {
                dbg!(self.space());
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
                    let block = code_block::read_from_right(self.space().sub(1)).1;
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
            // let left_size
            // let right_size
            // let left_next = free_space::set_left_next(self).unwrap();
            // let right_next = free_space::set_right_next(self).unwrap();

            // check data size
            if self.data_start() as usize >= self.data_end() as usize {
                dbg!(self.data_start());
                dbg!(self.data_end());
                panic!("data start and end are crossed")
            }
            // check space boundaries
            if (self.space() as usize) >= self.data_end() as usize
                || (self.data_start() as usize) > self.space() as usize
            {
                dbg!(self.data_start());
                dbg!(self.space());
                dbg!(self.data_end());
                panic!("space is outside of data")
            }
            if self.code_block_right.is_some() {
                if (self.code_block_right() as usize) < self.data_end() as usize
                    && (self.data_start() as usize) < self.code_block_right() as usize
                {
                    dbg!(self.data_start());
                    dbg!(self.code_block_right());
                    dbg!(self.data_end());
                    panic!("right codeblock outside of data boundaries")
                }
                // check both free bits are consistent
                let left_free = code_block::is_free(self.data_start());
                let right_free = code_block::is_free(self.code_block_right());
                if left_free != right_free {
                    dbg!(left_free);
                    dbg!(right_free);
                    panic!("free flags inconsistent")
                }
            }
        }
    }
    #[inline]
    pub fn check_next_boundaries(&self) {
        #[cfg(feature = "consistency-checks")]
        {
            unsafe {
                let next_target = free_space::get_next(self).unwrap();
                let start_of_page = (*self.page()).start_of_page() as *mut NextPointerType;
                let end_of_page = (*self.page()).end_of_page() as *mut NextPointerType;
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
    #[inline]
    pub fn check_left_ptr_boundary(&self, ptr: *mut u8) {
        #[cfg(feature = "consistency-checks")]
        {
            if self.data_start() > ptr {
                dbg!(self.data_start());
                dbg!(ptr);
                panic!("pointer not in allocation boundaries")
            }
        }
    }
    #[inline]
    pub fn check_right_ptr_boundary(&self, ptr: *mut u8) {
        #[cfg(feature = "consistency-checks")]
        {
            let data_end = match self.data_end {
                Some(end) => end,
                None => unsafe {
                    let data_size = self.calculate_data_size();
                    let code_block_size = code_block::get_block_size(self.data_start());
                    self.data_start().add(data_size + 2 * code_block_size)
                },
            };
            if data_end < ptr {
                dbg!(ptr);
                dbg!(data_end);
                panic!("pointer not in allocation boundaries")
            }
        }
    }
    // #[inline]
    // pub fn (&self) -> Result<(), MaraError> {
    //     #[cfg(feature = "consistency-checks")]
    //     {
    //     }
    //     Ok(())
    // }
    // #[inline]
    // pub fn (&self) -> Result<(), MaraError> {
    //     #[cfg(feature = "consistency-checks")]
    //     {
    //     }
    //     Ok(())
    // }
    // #[inline]
    // pub fn (&self) -> Result<(), MaraError> {
    //     #[cfg(feature = "consistency-checks")]
    //     {
    //     }
    //     Ok(())
    // }
    // #[inline]
    // pub fn (&self) -> Result<(), MaraError> {
    //     #[cfg(feature = "consistency-checks")]
    //     {
    //     }
    //     Ok(())
    // }
}
