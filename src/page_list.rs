use crate::code_block;
use crate::globals::*;
use crate::page::Page;
use crate::AllocationData;
use core::mem::size_of;

pub struct PageList {
    /// The first page in the ring that will be searched
    first_page: *mut Page,
    /// Size of the data array
    data_size: usize,
}

impl PageList {
    pub fn new(data: *mut u8, data_size: usize) -> Self {
        let max_code_block_size = code_block::get_needed_code_block_size(data_size);
        if data_size > NextPointerType::max_value() as usize - 2 * max_code_block_size {
            panic!(
                "Pages greater than {} bytes are not supported",
                NextPointerType::max_value() as usize - 2 * max_code_block_size
            );
        }
        // store the location to the first page
        let first_page = data as *mut Page;
        // after that push the data start right to reserve page objects space
        let data = unsafe { data.add(size_of::<Page>()) };
        let data_size = data_size - size_of::<Page>();
        unsafe { (*first_page).init(data, data_size) };
        unsafe { (*first_page).set_next_page(first_page) };
        Self {
            first_page,
            data_size: data_size,
        }
    }
    pub fn get_page(&self) -> *const Page {
        self.first_page
    }
    /// #### size_in_byte
    /// size of the block
    /// #### return
    /// a pointer to the block
    pub fn dynamic_new(&mut self, alloc_data: &mut AllocationData) {
        alloc_data.space.check_size(1, self.data_size);
        unsafe { (*self.first_page).get_dynamic_block(alloc_data) };
        #[cfg(feature = "statistic")]
        {
            byte * hurr = nullptr;
            Statistic::newDynamic(
                codeblock::read_from_right((start_of_space - 1), hurr),
                start_of_space,
            );
        }
    }
    /// frees a dynamic block
    /// #### address
    /// a pointer to the block
    pub fn dynamic_delete(&mut self, address: *mut u8) {
        let mut alloc_data = AllocationData::new();
        alloc_data.space.set_ptr(address);
        unsafe { (*self.first_page).delete_block(&mut alloc_data) };
    }
}
