use crate::code_block;
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
        self.ptr.expect("Space pointer was not cached earlier")
    }
    pub fn size(&self) -> usize {
        self.size.expect("Space size was not cached earlier")
    }
    /// load the cached pointer to a nother free space
    /// This is different from loading from memory (see ``load_next``)
    pub fn next(&self) -> Option<Space> {
        match self.next {
            None => panic!("next pointer was not cached earlier"),
            Some(ptr) if ptr == core::ptr::null_mut() => None,
            Some(ptr) => Some(Self {
                ptr: Some(ptr),
                size: None,
                next: None,
            }),
        }
    }
    pub fn set_ptr(&mut self, ptr: *mut u8) {
        self.ptr = Some(ptr);
    }
    pub fn set_size(&mut self, size: usize) {
        self.size = Some(size)
    }
    /// Cache ``free_space`` as next free space
    /// This is different from writing the pointer to memory (see ``write next``)
    pub fn set_next(&mut self, space: Option<Space>) {
        match space {
            None => self.next = Some(core::ptr::null_mut()),
            Some(space) => self.next = Some(space.ptr()),
        }
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
            match self.next() {
                None => *(self.ptr() as *mut NextPointerType) = ERROR_NEXT_POINTER,
                Some(next) => {
                    *(self.ptr() as *mut NextPointerType) =
                        (next.ptr().sub(start_of_page as usize)) as NextPointerType
                }
            }
        }
    }
    /// Reads the pointer that is stored at the location of ``ptr``
    /// The stored pointer is an offset from start of page.
    /// This is different from the cache method ``next``
    pub fn read_next(&self, start_of_page: *const u8) -> Option<Space> {
        unsafe {
            let next = *(self.ptr() as *mut NextPointerType);
            match next {
                ERROR_NEXT_POINTER => None,
                ptr => Some(Self {
                    ptr: Some(start_of_page.add(ptr as usize) as *mut u8),
                    size: None,
                    next: None,
                }),
            }
        }
    }
    pub fn cache_size_from_code_block(&mut self) {
        unsafe { self.set_size(code_block::read_from_right(self.ptr().sub(1)).0) }
    }
    /// Reads the next pointer at ``ptr`` and stores the encoded address in ``next``
    pub fn cache_next(&mut self, start_of_page: *const u8) {
        self.set_next(self.read_next(start_of_page))
    }

    /////////////////////////////////////////////
    // checks

    #[inline]
    pub fn check_size(&self, min: usize, max: usize) {
        #[cfg(feature = "consistency-checks")]
        {
            if self.size() < min {
                dbg!(self.size());
                dbg!(min);
                panic!("Space is smaller as expected");
            }
            if self.size() > max {
                dbg!(self.size());
                dbg!(max);
                panic!("space is larger as expected");
            }
        }
    }
}
