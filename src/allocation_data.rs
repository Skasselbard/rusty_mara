use crate::code_block;
use crate::free_space::NextPointerType;
use crate::MaraError;
use crate::Page;

#[derive(Debug, Copy, Clone)]
pub struct AllocationData {
    data_start: Option<*mut u8>,
    data_end: Option<*mut u8>,
    code_block_right: Option<*mut u8>,
    code_block_size: Option<usize>,
    space: Option<*mut u8>,
    space_size: Option<usize>,
    space_is_free: Option<bool>,
    next_pointer: Option<*mut NextPointerType>,
    page: Option<*mut Page>,
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
            space_is_free: None,
            next_pointer: None,
            page: None,
        }
    }

    ///////////////////////////////////////////////////
    //Getter
    pub fn data_start(&self) -> Result<*mut u8, MaraError> {
        match self.data_start {
            None => Err(MaraError::UninitializedAllocationData),
            Some(data) => Ok(data),
        }
    }
    pub fn data_end(&self) -> Result<*mut u8, MaraError> {
        match self.data_end {
            None => Err(MaraError::UninitializedAllocationData),
            Some(data) => Ok(data),
        }
    }
    pub fn code_block_right(&self) -> Result<*mut u8, MaraError> {
        match self.code_block_right {
            None => Err(MaraError::UninitializedAllocationData),
            Some(data) => Ok(data),
        }
    }
    pub fn code_block_size(&self) -> Result<usize, MaraError> {
        match self.code_block_size {
            None => Err(MaraError::UninitializedAllocationData),
            Some(data) => Ok(data),
        }
    }
    pub fn space(&self) -> Result<*mut u8, MaraError> {
        match self.space {
            None => Err(MaraError::UninitializedAllocationData),
            Some(data) => Ok(data),
        }
    }
    pub fn space_size(&self) -> Result<usize, MaraError> {
        match self.space_size {
            None => Err(MaraError::UninitializedAllocationData),
            Some(data) => Ok(data),
        }
    }
    pub fn space_is_free(&self) -> Result<bool, MaraError> {
        match self.space_is_free {
            None => Err(MaraError::UninitializedAllocationData),
            Some(data) => Ok(data),
        }
    }
    pub fn next_pointer(&self) -> Result<*mut NextPointerType, MaraError> {
        match self.next_pointer {
            None => Err(MaraError::UninitializedAllocationData),
            Some(data) => Ok(data),
        }
    }
    pub fn page(&self) -> Result<*mut Page, MaraError> {
        match self.page {
            None => Err(MaraError::UninitializedAllocationData),
            Some(data) => Ok(data),
        }
    }
    //////////////////////////////////////////////////////////
    // Generated data
    pub fn data_size(&self) -> Result<usize, MaraError> {
        Ok(self.data_end()? as usize - self.data_start()? as usize)
    }
    pub fn start_of_page(&self) -> Result<*const u8, MaraError> {
        Ok(unsafe { (*self.page()?).start_of_page() })
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
    pub fn set_space_free(&mut self, data: bool) {
        self.space_is_free = Some(data)
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
    pub fn check_space_size(&self, min: usize, max: usize) -> Result<(), MaraError> {
        #[cfg(feature = "consistency-checks")]
        {
            if self.space_size()? < min {
                dbg!(self.space_size()?);
                dbg!(min);
                return Err(MaraError::SpaceSizeToSmall);
            }
            if self.space_size()? > max {
                dbg!(self.space_size()?);
                dbg!(max);
                return Err(MaraError::SpaceSizeToBig);
            }
        }
        Ok(())
    }
    #[inline]
    pub fn check_data_size(&self, min: usize, max: usize) -> Result<(), MaraError> {
        #[cfg(feature = "consistency-checks")]
        {
            if self.data_size()? < min {
                dbg!(self.data_size()?);
                dbg!(min);
                return Err(MaraError::AllocSizeToSmall);
            }
            if self.data_size()? > max {
                dbg!(self.data_size()?);
                dbg!(max);
                return Err(MaraError::AllocSizeToBig);
            }
        }
        Ok(())
    }
    #[inline]
    pub fn check_space(&self) -> Result<(), MaraError> {
        #[cfg(feature = "consistency-checks")]
        {
            if self.space()?.is_null() {
                dbg!(self.space()?);
                return Err(MaraError::SpaceIsNull);
            }
        }
        Ok(())
    }
    #[inline]
    pub fn check_left_free(&self, expected: bool) -> Result<(), MaraError> {
        #[cfg(feature = "consistency-checks")]
        {
            if code_block::is_free(self.data_start()?) != expected {
                dbg!(code_block::is_free(self.data_start()?));
                dbg!(expected);
                return Err(MaraError::InconsistentCodeBlocks);
            }
        }
        Ok(())
    }
    #[inline]
    pub fn check_consistency(&self) -> Result<(), MaraError> {
        #[cfg(feature = "consistency-checks")]
        {
            let left_free = code_block::is_free(self.data_start()?);
            let right_free = code_block::is_free(self.code_block_right()?);
            // let left_size
            // let right_size
            // let left_next = free_space::set_left_next(self)?;
            // let right_next = free_space::set_right_next(self)?;
            if self.space_is_free()? != left_free || self.space_is_free()? != right_free {
                return Err(MaraError::InconsistentCodeBlocks);
            }
            if self.data_start()? as usize >= self.data_end()? as usize {
                return Err(MaraError::InconsistentAllocationData);
            }
            if (self.space()? as usize) < self.data_end()? as usize
                && (self.data_start()? as usize) < self.space()? as usize
            {
                return Err(MaraError::InconsistentAllocationData);
            }
            if (self.code_block_right()? as usize) < self.data_end()? as usize
                && (self.data_start()? as usize) < self.code_block_right()? as usize
            {
                return Err(MaraError::InconsistentAllocationData);
            }
        }
        Ok(())
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
    // #[inline]
    // pub fn (&self) -> Result<(), MaraError> {
    //     #[cfg(feature = "consistency-checks")]
    //     {
    //     }
    //     Ok(())
    // }
}
