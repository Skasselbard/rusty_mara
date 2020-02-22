use crate::code_block;
use crate::globals::*;
use crate::page::Page;
use crate::AllocationData;
use core::mem::size_of;

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
        let max_code_block_size = code_block::get_needed_code_block_size(page_size);
        if page_size > NextPointerType::max_value() as usize - 2 * max_code_block_size {
            panic!(
                "Pages greater than {} bytes are not supported",
                NextPointerType::max_value() as usize - 2 * max_code_block_size
            );
        }
        // store the location to the first page
        let first_page = data as *mut Page;
        // after that push the data start right to reserve page objects space
        let data = unsafe { data.offset((list_memory_size * size_of::<*mut Page>()) as isize) };
        unsafe { (*first_page).init(data, page_size) };
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
    pub fn get_first_page(&self) -> *const Page {
        self.first_page
    }
    /// #### size_in_byte
    /// size of the block
    /// #### return
    /// a pointer to the block
    pub fn dynamic_new(&mut self, alloc_data: &mut AllocationData) {
        alloc_data.check_space_size(1, self.page_size);
        let current_page = self.first_page;
        loop {
            unsafe { (*current_page).get_dynamic_block(alloc_data) };
            if alloc_data.space.is_some() {
                break;
            }
            self.iterate_page(current_page);
        }
        #[cfg(feature = "statistic")]
        {
            byte * hurr = nullptr;
            Statistic::newDynamic(
                codeblock::read_from_right((start_of_space - 1), hurr),
                start_of_space,
            );
        }
        unsafe { (*current_page).check_alloc_start(alloc_data) };
        self.first_page = current_page;
    }
    /// frees a dynamic block
    /// #### address
    /// a pointer to the block
    pub fn dynamic_delete(&mut self, address: *mut u8) {
        let mut alloc_data = AllocationData::new();
        alloc_data.space.set_ptr(address);
        let current_page = unsafe { &mut *(self.first_page) };
        while !current_page.block_is_in_space(address) {
            self.iterate_page(current_page);
        }
        current_page.delete_block(&mut alloc_data);
    }
    ///Inserts a new page into cyclic list after the specified page.
    ///@param current_page the page after which a new page will be inserted
    ///@return {@code true} if a new page was successfully inserted.
    ///
    fn add_page_to_list(&mut self, current_page: &mut Page) {
        if self.page_count < self.list_memory_size {
            let offset = self.page_count * self.page_size;
            if offset + self.page_size >= self.data_size {
                panic!("Mara: Out of Memory")
            };
            let next_page = unsafe { self.data.offset(offset as isize) as *mut Page };
            self.page_count = self.page_count + 1;
            unsafe { (*next_page).set_next_page(current_page.get_next_page()) };
            current_page.set_next_page(next_page);
        } else {
            panic!("Mara: Out of Pages")
        }
    }
    /// rotates the cyclic list one step. if it has reached the end it adds the page to the list
    /// #### current_page
    /// the current page
    /// #### return
    /// if the addition was successful
    #[inline]
    fn iterate_page(&mut self, current_page: *mut Page) -> *const Page {
        let mut current_page = unsafe { &mut *current_page };
        if current_page.get_next_page() == self.first_page {
            self.add_page_to_list(&mut current_page);
        }
        current_page.get_next_page()
    }
}
