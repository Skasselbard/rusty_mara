use crate::page::Page;
use crate::AllocationData;
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
    pub fn new(
        page_size: usize,
        data: *mut u8,
        data_size: usize,
        list_memory_size: usize,
    ) -> Result<Self, MaraError> {
        // push the page memory left to fit in the page objects
        let data = unsafe { data.offset((list_memory_size * size_of::<*mut Page>()) as isize) };
        // initialize the first page
        let first_page = data as *mut Page;
        unsafe { (*first_page).init(data, page_size)? };
        unsafe { (*first_page).set_next_page(first_page) };
        Ok(Self {
            page_size,
            first_page,
            page_count: 1,
            list_memory_size,
            data,
            data_size: data_size - list_memory_size,
        })
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

    /// #### size_in_byte
    /// size of the block
    /// #### return
    /// a pointer to the block
    pub fn dynamic_new(&mut self, alloc_data: &mut AllocationData) -> Result<(), MaraError> {
        alloc_data.check_space_size(1, self.page_size)?;
        let current_page = self.first_page;
        loop {
            unsafe { (*current_page).get_dynamic_block(alloc_data)? };
            if alloc_data.data_start()?.is_null() {
                break;
            }
            self.iterate_page(current_page)?;
        }
        #[cfg(feature = "statistic")]
        {
            byte * hurr = nullptr;
            Statistic::newDynamic(
                codeblock::read_from_right((start_of_space - 1), hurr),
                start_of_space,
            );
        }
        #[cfg(feature = "consistency-checks")]
        {
            unsafe {
                assert!(alloc_data.data_start()? >= (*current_page).start_of_page() as *mut u8);
            }
        }
        self.first_page = current_page;
        Ok(())
    }
    /// frees a dynamic block
    /// #### address
    /// a pointer to the block
    pub fn dynamic_delete(&mut self, address: *mut u8) -> Result<(), MaraError> {
        let mut alloc_data = AllocationData::new();
        alloc_data.set_space(address);
        let current_page = unsafe { &mut *(self.first_page) };
        while !current_page.block_is_in_space(address) {
            self.iterate_page(current_page)?;
        }
        current_page.delete_block(&mut alloc_data)?;
        Ok(())
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
