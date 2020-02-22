/// Basic Structure:
/// ```
/// Standard Free Space (assuming a next pointer size of 4 byte = 32 bit):
/// ------------------------------------------------------------------------------------
/// |.CodeBlock.|.next_pointer.|.........Free Space...........|.next_pointer.|.CodeBlock.|
/// |.min 1byte.|....4byte....|.max PAGE_SIZE - 10 byte byte.|....4byte....|.min 1byte.|
/// ------------------------------------------------------------------------------------
///
/// 6byte Free Space:
/// ---------------------------------------
/// |.CodeBlock.|.next_pointer.|.CodeBlock.|
/// |.min 1byte.|....4byte....|.min 1byte.|
/// ---------------------------------------
///
/// Occupied space Space:
/// ------------------------------------------------------------------
/// |.CodeBlock.|...................Data.................|.CodeBlock.|
/// |.min 1byte.|6byte to (max PAGE_SIZE - 10 byte) byte |.min 1byte.|
/// ------------------------------------------------------------------
/// ```
use crate::globals::*;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Space {
    /// The beginning of a space.
    /// This pointer will be returnded by mara.
    /// Succeeds the left code block.
    /// Precedes the right code block
    /// If this is a free space, this location is used to store the next pointer.
    ptr: Option<*mut u8>,
    /// Size of the space in bytes.
    /// Depending on allocation or deallocation this is an intended size.
    /// or an actual size.
    /// The right code block is located at space + space_size.
    size: Option<usize>,
    /// A pointer to the next free space.
    /// Points to another next pointer (NOT the data start of another allocation).
    /// Null if there is no successor.
    next: Option<*mut u8>,
}

impl Space {
    pub fn new() -> Self {
        Self {
            ptr: None,
            size: None,
            next: None,
        }
    }
    pub fn ptr(&self) -> *mut u8 {
        self.ptr.expect("Space pointer was not cached before")
    }
    pub fn size(&self) -> usize {
        self.size.expect("Space pointer was not cached before")
    }
    /// load the cached pointer to a nother free space
    /// This is different from loading from memory (see ``load_next``)
    pub fn next(&self) -> *mut u8 {
        self.next.expect("next pointer was not cached before")
    }
    pub fn set_ptr(&mut self, ptr: *mut u8) {
        self.ptr = Some(ptr);
    }
    pub fn set_size(&mut self, size: usize) {
        self.size = Some(size)
    }
    /// Cache ``free_space`` as next free space
    /// This is different from writing the pointer to memory (see ``write next``)
    pub fn set_next(&mut self, free_space: *mut u8) {
        self.next = Some(free_space);
    }
    pub fn is_some(&self) -> bool {
        self.ptr.is_some()
    }
    pub fn size_is_some(&self) -> bool {
        self.size.is_some()
    }
    /// Writes the pointer stored in ``next`` to the location ``ptr`` points to
    /// The stored pointer will be an offset from start of page.
    /// This is different form the cache method ``set_next``
    pub fn write_next(&mut self, start_of_page: *const u8) {
        unsafe {
            if self.next() == core::ptr::null_mut() {
                *(self.ptr() as *mut NextPointerType) = ERROR_NEXT_POINTER;
            } else {
                *(self.ptr() as *mut NextPointerType) =
                    (self.next().sub(start_of_page as usize)) as NextPointerType;
            }
        }
    }
    /// Reads the pointer that is stored at the location of ``ptr``
    /// The stored pointer is an offset from start of page.
    /// This is different from the cache method ``next``
    pub fn read_next(&self, start_of_page: *const u8) -> *mut u8 {
        unsafe {
            let next = *(self.ptr() as *mut NextPointerType);
            if next == ERROR_NEXT_POINTER {
                core::ptr::null_mut()
            } else {
                start_of_page.add(next as usize) as *mut u8
            }
        }
    }
}
