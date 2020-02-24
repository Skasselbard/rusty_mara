#[cfg_attr(feature = "no_std", no_std)]
extern crate alloc;

mod allocation_data;
mod bucket_list;
mod code_block;
mod consistency;
mod globals;
mod page;
mod page_list;
mod space;

#[cfg(feature = "consistency_tests")]
pub use consistency::TestBuilder;

use alloc::alloc::{GlobalAlloc, Layout};
use allocation_data::AllocationData;
use core::cell::UnsafeCell;
use core::mem::transmute;
use page::Page;
use page_list::PageList;

pub struct Mara {
    page_list: UnsafeCell<PageList>,
}

impl Mara {
    /// #### data
    /// start of data array
    /// #### data_size
    /// length of the data array in bytes
    pub fn new(data: *mut u8, data_size: usize) -> Self {
        if data_size > globals::MAX_PAGE_SIZE {
            panic!("Mara: Max page size is {} bytes", globals::MAX_PAGE_SIZE);
        }
        let page_list = UnsafeCell::new(PageList::new(data, data_size));
        Self { page_list }
    }

    pub(crate) fn page_list(&self) -> &mut PageList {
        unsafe { transmute::<*mut PageList, &mut PageList>(self.page_list.get()) }
    }
    /// Reserves memory in the static sector. Memory in this sector is expected to live as long as Mara. Memory
    /// allocated with this function CANNOT be freed. Mara returns a pointer to the location with an unused block with the
    /// given size and completely ignore this space in the future. The advantage is that these blocks will produce absolutely
    /// no additional.
    /// #### size_in_byte
    /// size of the block you want to use
    /// #### return
    /// a pointer to the first byte of the block you want to use. After this operation the block will stay allocated
    /// until complete program termination.
    pub fn static_new(&self, _size_in_byte: usize) -> *mut u8 {
        unimplemented!();
        //self.page_list().static_new(size_in_byte)
    }

    /**
     * Reserves memory in the dynamic sector. Memory in this sector can be freed using the dynamicDelete method.
     * @param size_in_byte how many bytes shall be reserved
     * @return a pointer to the first byte in a reserved space with at least the requested size
     */
    pub fn dynamic_new(&self, size_in_byte: usize) -> *mut u8 {
        let mut allocation_data = AllocationData::new();
        allocation_data.space.set_size(size_in_byte);
        self.page_list().dynamic_new(&mut allocation_data);
        allocation_data.space.ptr()
    }

    /**
     * frees a previously reserved space in the dynamic sector
     * @param address the pointer that was returned by dynamicNew
     * @return true if the operation was successful, false elsewhen
     */
    pub fn dynamic_delete(&self, address: *mut u8) {
        self.page_list().dynamic_delete(address)
    }
}

unsafe impl GlobalAlloc for Mara {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.dynamic_new(layout.size())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        self.dynamic_delete(ptr);
    }
}
