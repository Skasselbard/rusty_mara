#[cfg_attr(feature = "no_std", no_std)]
extern crate alloc;

mod bucketlist;
mod codeblock;
mod freespace;
mod globals;
mod page;
mod pagelist;
mod space;

use alloc::alloc::{GlobalAlloc, Layout};
use core::mem::transmute;
use core::cell::UnsafeCell;
use pagelist::PageList;

pub enum MaraError {
    OutOfMemory,
    OutOfPages,
}

pub struct Mara {
    page_list: UnsafeCell<PageList>,
}

impl Mara {
    fn page_list(&self) -> &mut PageList{
        unsafe{transmute::<*mut PageList, &mut PageList>(self.page_list.get())}
    }
    /// Reserves memory in the static sector. Memory in this sector is expected to live as long as Mara. Memory
    /// allocated with this function CANNOT be freed. Mara returns a pointer to the location with an unused block with the
    /// given size and completely ignore this space in the future. The advantage is that these blocks will produce absolutely
    /// no additional.
    /// #### sizeInByte
    /// size of the block you want to use
    /// #### return
    /// a pointer to the first byte of the block you want to use. After this operation the block will stay allocated
    /// until complete program termination.
    pub fn staticNew(&self, sizeInByte: usize) -> *mut u8 {
        self.page_list().staticNew(sizeInByte)
    }

    /**
     * Reserves memory in the dynamic sector. Memory in this sector can be freed using the dynamicDelete method.
     * @param sizeInByte how many bytes shall be reserved
     * @return a pointer to the first byte in a reserved space with at least the requested size
     */
    pub fn dynamicNew(&self, layout: Layout) -> *mut u8 {
        self.page_list().dynamicNew(layout.size())
    }

    /**
     * frees a previously reserved space in the dynamic sector
     * @param address the pointer that was returned by dynamicNew
     * @return true if the operation was successful, false elsewhen
     */
    pub fn dynamicDelete(&self, address: *mut u8, _layout: Layout) {
        match self.page_list().dynamicDelete(address) {
            false => panic!("deallocation failed"),
            true => {}
        }
    }
}

unsafe impl GlobalAlloc for Mara {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.dynamicNew(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.dynamicDelete(ptr, layout);
    }
}
