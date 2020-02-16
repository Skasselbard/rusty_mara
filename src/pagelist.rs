use crate::page::Page;
use crate::space::*;
use crate::MaraError;
use core::mem::size_of;
use core::result::Result;

pub struct PageList {
    pageSize: usize,
    /// The first page in the ring that will be searched
    firstPage: *mut Page,
    /// amount of pages we have created
    page_count: usize,
    /// Here all page objects are stored
    list_memory: *mut Page,
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
    pub fn new(pageSize: usize, data: *mut u8, data_size: usize, list_memory_size: usize) -> Self {
        let list_memory = data as *mut Page;
        let data = unsafe { data.offset((list_memory_size * size_of::<*mut Page>()) as isize) };
        let firstPage = unsafe { (&mut (*list_memory).init(data, pageSize)) as *mut Page };
        unsafe { (*firstPage).setNextPage(firstPage) };
        Self {
            pageSize,
            firstPage,
            page_count: 1,
            list_memory,
            list_memory_size,
            data,
            data_size: data_size - list_memory_size,
        }
    }
    /// sets the page size
    /// #### sizeInByte
    /// the page size
    pub fn setPageSize(&mut self, sizeInByte: usize) {
        self.pageSize = sizeInByte
    }

    pub fn getPageSize(&self) -> usize {
        self.pageSize
    }

    pub fn getFirstPage(&self) -> *const Page {
        self.firstPage
    }

    /// reserves a new static block
    /// static blocks cannot be deleted but can be stored without additional management information
    /// #### sizeInByte
    /// size of the block
    /// #### return
    /// a pointer to the block
    pub fn staticNew(&mut self, sizeInByte: usize) -> *mut u8 {
        #[cfg(feature = "condition")]
        {
            assert!(sizeInByte != 0); //self-explainatory
        }
        let currentPage = self.firstPage;
        let mut returnBlock;
        loop {
            returnBlock = unsafe { (*currentPage).getStaticBlock(sizeInByte) };
            if returnBlock.is_null() {
                break;
            }
            if self.iteratePage(currentPage).is_err() {
                returnBlock = core::ptr::null_mut();
                break;
            }
        }
        #[cfg(feature = "statistic")]
        {
            Statistic::newStatic(sizeInByte, returnBlock);
        }
        #[cfg(feature = "condition")]
        {
            unsafe {
                assert!(returnBlock == (*currentPage).getStaticEnd() as *mut u8); //the returned block must be at the top of the static area
                assert!(
                    returnBlock as usize + sizeInByte
                        <= (*currentPage).getStartOfPage() as usize + self.pageSize
                );
                //the returned block may not go over the page boundaries
            }
        }
        self.firstPage = currentPage;
        return returnBlock;
    }
    /// #### sizeInByte
    /// size of the block
    /// #### return
    /// a pointer to the block
    pub fn dynamicNew(&mut self, sizeInByte: usize) -> *mut u8 {
        #[cfg(feature = "condition")]
        {
            assert!(sizeInByte > 0);
        }
        let currentPage = self.firstPage;
        let mut returnBlock;
        loop {
            returnBlock = unsafe { (*currentPage).getDynamicBlock(sizeInByte) };
            if returnBlock.is_null() {
                break;
            }
            if self.iteratePage(currentPage).is_err() {
                return core::ptr::null_mut();
            }
        }
        let startOfSpace = getStartOfSpace(returnBlock);
        #[cfg(feature = "statistic")]
        {
            byte * hurr = nullptr;
            Statistic::newDynamic(
                codeblock::readFromRight((startOfSpace - 1), hurr),
                startOfSpace,
            );
        }
        #[cfg(feature = "condition")]
        {
            unsafe {
                assert!(
                    returnBlock >= (*currentPage).getStartOfPage() as *mut u8
                        && returnBlock < (*currentPage).getStaticEnd() as *mut u8,
                );
            }
        }
        self.firstPage = currentPage;
        startOfSpace
    }
    /// frees a dynamic block
    /// #### address
    /// a pointer to the block
    /// #### return
    /// if it was successful
    pub fn dynamicDelete(&mut self, address: *const u8) -> bool {
        let mut currentPage = unsafe { &mut *(self.firstPage) };
        while !currentPage.blockIsInSpace(address) {
            if self.iteratePage(currentPage).is_err() {
                return false;
            }
        }
        currentPage.deleteBlock(address);
        return true;
    }
    /// #### return
    /// how many pages are in the cyclic list
    pub fn getPageCount(&self) -> usize {
        let mut count = 0;
        let mut page = unsafe { &mut *self.firstPage };
        loop {
            count = count + 1;
            page = unsafe { &mut *page.getNextPage() };
            if page as *mut Page == self.firstPage {
                break;
            }
        }
        count
    }
    ///Inserts a new page into cyclic list after the specified page.
    ///@param currentPage the page after which a new page will be inserted
    ///@return {@code true} if a new page was successfully inserted.
    ///
    fn addPageToList(&mut self, currentPage: &mut Page) -> Result<(), MaraError> {
        if self.page_count < self.list_memory_size {
            let offset = self.page_count * self.pageSize;
            if offset + self.pageSize >= self.data_size {
                return Err(MaraError::OutOfMemory);
            };
            let nextPage = unsafe { self.data.offset(offset as isize) as *mut Page };
            self.page_count = self.page_count + 1;
            unsafe { (*nextPage).setNextPage(currentPage.getNextPage()) };
            currentPage.setNextPage(nextPage);
            Ok(())
        } else {
            Err(MaraError::OutOfPages)
        }
    }
    /// rotates the cyclic list one step. if it has reached the end it adds the page to the list
    /// #### currentPage
    /// the current page
    /// #### return
    /// if the addition was successful
    fn iteratePage(&mut self, currentPage: *mut Page) -> Result<*const Page, MaraError> {
        let mut currentPage = unsafe { &mut *currentPage };
        if currentPage.getNextPage() == self.firstPage {
            self.addPageToList(&mut currentPage)?;
            Ok(currentPage.getNextPage())
        } else {
            Ok(currentPage.getNextPage())
        }
    }
}
