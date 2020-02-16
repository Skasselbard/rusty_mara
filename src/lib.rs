#[cfg_attr(feature = "no_std", no_std)]
extern crate alloc;

mod bucket_list;
mod code_block;
mod free_space;
mod globals;
mod page;
mod page_list;
mod space;

use alloc::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::mem::transmute;
use page_list::PageList;

pub enum MaraError {
    OutOfMemory,
    OutOfPages,
}

pub struct Mara {
    page_list: UnsafeCell<PageList>,
}

impl Mara {
    pub fn new(page_size: usize, data: *mut u8, data_size: usize, list_memory_size: usize) -> Self {
        let page_list =
            UnsafeCell::new(PageList::new(page_size, data, data_size, list_memory_size));
        Self { page_list }
    }

    fn page_list(&self) -> &mut PageList {
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
    pub fn static_new(&self, size_in_byte: usize) -> *mut u8 {
        self.page_list().static_new(size_in_byte)
    }

    /**
     * Reserves memory in the dynamic sector. Memory in this sector can be freed using the dynamicDelete method.
     * @param size_in_byte how many bytes shall be reserved
     * @return a pointer to the first byte in a reserved space with at least the requested size
     */
    pub fn dynamic_new(&self, layout: Layout) -> *mut u8 {
        self.page_list().dynamic_new(layout.size())
    }

    /**
     * frees a previously reserved space in the dynamic sector
     * @param address the pointer that was returned by dynamicNew
     * @return true if the operation was successful, false elsewhen
     */
    pub fn dynamic_delete(&self, address: *mut u8, _layout: Layout) {
        match self.page_list().dynamic_delete(address) {
            false => panic!("deallocation failed"),
            true => {}
        }
    }
}

unsafe impl GlobalAlloc for Mara {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.dynamic_new(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.dynamic_delete(ptr, layout);
    }
}
