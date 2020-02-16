use crate::page::Page;
use crate::space::*;
use crate::MaraError;
use core::mem::size_of;
use core::result::Result;

pub struct PageList {
    page_size: usize,
    /// The first page in the ring that will be searched
    first_page: *mut Page,
    /// amount of pages we have created
    page_count: usize,
    /// The size of the memory in pages (not in bytes)
    /// If the size is 100, 100 pages can be stored
    list_memory_size: usize,
    /// All things that have to be stored
    /// This includes the heap and all object data we need for the heap
    /// On PageList creation some of the data was reserved for the page list
    data: *mut u8,
    /// Size of the data array
    data_size: usize,
}

impl PageList {
    pub fn new(page_size: usize, data: *mut u8, data_size: usize, list_memory_size: usize) -> Self {
        let data = unsafe { data.offset((list_memory_size * size_of::<*mut Page>()) as isize) };
        let first_page =
            unsafe { (&mut (*(data as *mut Page)).init(data, page_size)) as *mut Page };
        unsafe { (*first_page).set_next_page(first_page) };
        Self {
            page_size,
            first_page,
            page_count: 1,
            list_memory_size,
            data,
            data_size: data_size - list_memory_size,
        }
    }
    /// sets the page size
    /// #### size_in_byte
    /// the page size
    pub fn set_page_size(&mut self, size_in_byte: usize) {
        self.page_size = size_in_byte
    }

    pub fn get_page_size(&self) -> usize {
        self.page_size
    }

    pub fn get_first_page(&self) -> *const Page {
        self.first_page
    }

    /// reserves a new static block
    /// static blocks cannot be deleted but can be stored without additional management information
    /// #### size_in_byte
    /// size of the block
    /// #### return
    /// a pointer to the block
    pub fn static_new(&mut self, size_in_byte: usize) -> *mut u8 {
        #[cfg(feature = "condition")]
        {
            assert!(size_in_byte != 0); //self-explainatory
        }
        let current_page = self.first_page;
        let mut return_block;
        loop {
            return_block = unsafe { (*current_page).get_static_block(size_in_byte) };
            if return_block.is_null() {
                break;
            }
            if self.iterate_page(current_page).is_err() {
                return_block = core::ptr::null_mut();
                break;
            }
        }
        #[cfg(feature = "statistic")]
        {
            Statistic::newStatic(size_in_byte, return_block);
        }
        #[cfg(feature = "condition")]
        {
            unsafe {
                assert!(return_block == (*current_page).get_static_end() as *mut u8); //the returned block must be at the top of the static area
                assert!(
                    return_block as usize + size_in_byte
                        <= (*current_page).get_start_of_page() as usize + self.page_size
                );
                //the returned block may not go over the page boundaries
            }
        }
        self.first_page = current_page;
        return return_block;
    }
    /// #### size_in_byte
    /// size of the block
    /// #### return
    /// a pointer to the block
    pub fn dynamic_new(&mut self, size_in_byte: usize) -> *mut u8 {
        #[cfg(feature = "condition")]
        {
            assert!(size_in_byte > 0);
        }
        let current_page = self.first_page;
        let mut return_block;
        loop {
            return_block = unsafe { (*current_page).get_dynamic_block(size_in_byte) };
            if return_block.is_null() {
                break;
            }
            if self.iterate_page(current_page).is_err() {
                return core::ptr::null_mut();
            }
        }
        let start_of_space = get_start_of_space(return_block);
        #[cfg(feature = "statistic")]
        {
            byte * hurr = nullptr;
            Statistic::newDynamic(
                codeblock::read_from_right((start_of_space - 1), hurr),
                start_of_space,
            );
        }
        #[cfg(feature = "condition")]
        {
            unsafe {
                assert!(
                    return_block >= (*current_page).get_start_of_page() as *mut u8
                        && return_block < (*current_page).get_static_end() as *mut u8,
                );
            }
        }
        self.first_page = current_page;
        start_of_space
    }
    /// frees a dynamic block
    /// #### address
    /// a pointer to the block
    /// #### return
    /// if it was successful
    pub fn dynamic_delete(&mut self, address: *const u8) -> bool {
        let current_page = unsafe { &mut *(self.first_page) };
        while !current_page.block_is_in_space(address) {
            if self.iterate_page(current_page).is_err() {
                return false;
            }
        }
        current_page.delete_block(address);
        return true;
    }
    /// #### return
    /// how many pages are in the cyclic list
    pub fn get_page_count(&self) -> usize {
        let mut count = 0;
        let mut page = unsafe { &mut *self.first_page };
        loop {
            count = count + 1;
            page = unsafe { &mut *page.get_next_page() };
            if page as *mut Page == self.first_page {
                break;
            }
        }
        count
    }
    ///Inserts a new page into cyclic list after the specified page.
    ///@param current_page the page after which a new page will be inserted
    ///@return {@code true} if a new page was successfully inserted.
    ///
    fn add_page_to_list(&mut self, current_page: &mut Page) -> Result<(), MaraError> {
        if self.page_count < self.list_memory_size {
            let offset = self.page_count * self.page_size;
            if offset + self.page_size >= self.data_size {
                return Err(MaraError::OutOfMemory);
            };
            let next_page = unsafe { self.data.offset(offset as isize) as *mut Page };
            self.page_count = self.page_count + 1;
            unsafe { (*next_page).set_next_page(current_page.get_next_page()) };
            current_page.set_next_page(next_page);
            Ok(())
        } else {
            Err(MaraError::OutOfPages)
        }
    }
    /// rotates the cyclic list one step. if it has reached the end it adds the page to the list
    /// #### current_page
    /// the current page
    /// #### return
    /// if the addition was successful
    #[inline]
    fn iterate_page(&mut self, current_page: *mut Page) -> Result<*const Page, MaraError> {
        let mut current_page = unsafe { &mut *current_page };
        if current_page.get_next_page() == self.first_page {
            self.add_page_to_list(&mut current_page)?;
            Ok(current_page.get_next_page())
        } else {
            Ok(current_page.get_next_page())
        }
    }
}
