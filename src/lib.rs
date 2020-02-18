#[cfg_attr(feature = "no_std", no_std)]
extern crate alloc;

mod allocation_data;
mod bucket_list;
mod code_block;
mod consistency;
mod free_space;
mod globals;
mod page;
mod page_list;
mod space;

pub use consistency::TestBuilder;

use alloc::alloc::{GlobalAlloc, Layout};
use allocation_data::AllocationData;
use core::cell::UnsafeCell;
use core::mem::{size_of, transmute};
use page::Page;
use page_list::PageList;
use rand;

#[derive(Debug, Copy, Eq, PartialEq, Clone)]
pub enum MaraError {
    OutOfMemory,
    OutOfPages,
    NotEnoughMemory,
    UninitializedAllocationData,
    NoFittingSpace,
    AllocationNotFound,
    CodeBlockOverflow,

    SpaceSizeToSmall,
    SpaceSizeToBig,
    SpaceIsNull,
    AllocSizeToSmall,
    AllocSizeToBig,
    PageOverflow,
    InconsistentPage,
    InconsistentCodeBlocks,
    InconsistentAllocationData,
}

pub struct Mara {
    page_list: UnsafeCell<PageList>,
}

impl Mara {
    /// #### data
    /// start of data array
    /// #### data_size
    /// length of the data array in bytes
    pub fn new(page_size: usize, data: *mut u8, data_size: usize) -> Result<Self, MaraError> {
        // compute how many pages fit in the memory and
        // consider the size that is needed to store the page objects
        let mut max_pages = data_size / page_size;
        let mut page_object_data_size = max_pages * size_of::<Page>();
        while (data_size - page_object_data_size) / page_size != max_pages {
            max_pages = max_pages - 1;
            page_object_data_size = page_object_data_size - 1;
        }
        if max_pages <= 0 {
            Err(MaraError::NotEnoughMemory)
        } else {
            let page_list = UnsafeCell::new(PageList::new(page_size, data, data_size, max_pages)?);
            Ok(Self { page_list })
        }
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
    pub fn static_new(&self, size_in_byte: usize) -> *mut u8 {
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
        allocation_data.set_space_size(size_in_byte);
        self.page_list()
            .dynamic_new(&mut allocation_data)
            .expect("Allocation Error");
        allocation_data.space().unwrap()
    }

    /**
     * frees a previously reserved space in the dynamic sector
     * @param address the pointer that was returned by dynamicNew
     * @return true if the operation was successful, false elsewhen
     */
    pub fn dynamic_delete(&self, address: *mut u8) {
        self.page_list()
            .dynamic_delete(address)
            .expect("deallocation failed")
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
