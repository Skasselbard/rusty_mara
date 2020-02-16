use core::mem::{size_of_val, transmute};

use crate::bucket_list::BucketList;
use crate::code_block;
use crate::free_space::*;
use crate::globals::*;
use crate::space::*;

pub struct Page {
    /// Pointer to the first byte of the page
    start_of_page: *const u8,
    /// Pointer to the next page
    next_page: *mut Self,
    /// pointer to the leftmost byte of the static sector <br/>
    /// the rightmost byte is the last byte of the page
    static_end: *const u8,
    ///pointer to the rightmost allocated byte of the dynamic sector <br/>
    ///behind this pointer can only be an allocated chunk form the static
    ///sector. space between this pointer and the static_end pointer has to be free memory.
    dynamic_end: *const u8,
    bucket_list: BucketList,
}

impl Page {
    pub fn init(&mut self, page_memory: *mut u8, page_size: usize) -> Self {
        unsafe {
            code_block::set_free(page_memory, true);
            let bucket_list_memory =
                *transmute::<*mut u8, *mut [*mut u8; BUCKET_LIST_SIZE]>(page_memory);
            let bucket_list_size = size_of_val(&bucket_list_memory);
            let start_of_page = page_memory.offset(bucket_list_size as isize);
            let mut bucket_list = BucketList::new(bucket_list_memory, page_memory);
            bucket_list.add_to_list(Self::generate_first_bucket_entry(
                start_of_page,
                page_memory.offset(page_size as isize),
            ));
            #[cfg(feature = "condition")]
            {
                for i in 0..(BUCKET_LIST_SIZE - 1) {
                    assert!(bucket_list.get_from_bucket_list(i).is_null());
                }
                let block_size = code_block::get_block_size(
                    bucket_list.get_from_bucket_list(BUCKET_LIST_SIZE - 1),
                );
                assert!(
                    code_block::read_from_left(
                        bucket_list.get_from_bucket_list(BUCKET_LIST_SIZE - 1)
                    ) == page_size - 2 * block_size,
                );
            }
            Self {
                next_page: core::ptr::null_mut(),
                start_of_page,
                static_end: page_memory.offset(page_size as isize),
                dynamic_end: page_memory,
                bucket_list,
            }
        }
    }
    ////returns a new static block
    pub unsafe fn get_static_block(&mut self, size_in_byte: usize) -> *mut u8 {
        #[cfg(feature = "condition")]
        {
            assert!(size_in_byte != 0);
            assert!(self.static_end > self.dynamic_end); //static end should come after the dynamic end
        }
        #[cfg(feature = "align_static")]
        {
            size_in_byte = Self::align(size_in_byte);
        }

        if self.static_block_fit_in_page(size_in_byte) {
            let (code_block_size, _) = code_block::read_from_right(self.static_end.offset(-1));
            let last_free_space =
                self.static_end.offset(-(3 * code_block_size as isize)) as *mut u8;
            self.bucket_list.delete_from_list(last_free_space);
            self.cut_right_from_free_space(last_free_space, size_in_byte);
            self.bucket_list.add_to_list(last_free_space); //last_free_space might get too small for its current bucket
            self.static_end = self.static_end.offset(-(size_in_byte as isize));
            #[cfg(feature = "condition")]
            {
                assert!(self.static_end > self.dynamic_end); //see above
            }
            return self.static_end as *mut u8;
        } else {
            #[cfg(feature = "condition")]
            {
                assert!(self.static_end > self.dynamic_end); //see above
                assert!((self.static_end as usize - self.dynamic_end as usize) < 6 + size_in_byte);
                //there actually shouldn't be enough space
            }
            return core::ptr::null_mut();
        }
    }
    ///returns if a requested block size would fit in the page
    ///checks if there is enough space to begin with and if there would be enough space for a freespace(>6 byte) after insertion
    pub fn static_block_fit_in_page(&self, block_size_in_byte: usize) -> bool {
        //no assertions because state isn't altered
        (block_size_in_byte <= (self.static_end as usize - self.dynamic_end as usize - 1)
            && (self.static_end as usize - self.dynamic_end as usize >= 6 + block_size_in_byte))
    }
    /// tries to reserve a dynamic block in this page, and returns it
    /// #### size_in_byte
    /// the size of the space requested
    /// #### return
    /// a pointer to the space, or nullptr if no space was found
    pub fn get_dynamic_block(&mut self, size_in_byte: usize) -> *mut u8 {
        #[cfg(feature = "condition")]
        {
            assert!(size_in_byte > 0);
            assert!(self.static_end > self.dynamic_end);
        }
        #[cfg(feature = "align_dynamic")]
        {
            size_in_byte = align(size_in_byte);
        }
        let free_space = unsafe { self.bucket_list.get_free_space(size_in_byte) };
        let return_block = free_space;
        if free_space.is_null() {
            #[cfg(feature = "condition")]
            {
                assert!(self.static_end > self.dynamic_end);
            }
            return core::ptr::null_mut();
        } else {
            unsafe { self.bucket_list.delete_from_list(free_space) };
            let remaining_space = self.cut_left_from_free_space(
                free_space,
                size_in_byte + (2 * code_block::get_needed_code_block_size(size_in_byte)),
            );
            if !remaining_space.is_null() {
                unsafe { self.bucket_list.add_to_list(remaining_space) };
                unsafe { to_occupied(return_block, size_in_byte) };
            } else {
                //Edge Case: If the remaining space is too small to be used again, simply return a larger block
                unsafe { code_block::set_free(return_block, false) };
                unsafe {
                    copy_code_block_to_end(return_block, code_block::get_block_size(return_block))
                };
            }

            if get_right_most_end(return_block) > self.dynamic_end {
                self.dynamic_end = get_right_most_end(return_block);
            }
        }
        #[cfg(feature = "condition")]
        {
            assert!(!return_block.is_null());
            assert!(self.dynamic_end < self.static_end);
            assert!(self.dynamic_end > self.start_of_page);
            assert!(return_block >= self.start_of_page as *mut u8);
            assert!(!code_block::is_free(return_block));
        }
        return_block
    }
    /// #### return the next page in the ring storage
    pub fn get_next_page(&self) -> *mut Self {
        self.next_page
    }
    /// sets the next page
    /// #### next_page
    /// the next page
    pub fn set_next_page(&mut self, next_page: *mut Self) {
        if next_page != core::ptr::null_mut() {}
        self.next_page = next_page;
    }
    /// #### first_byte
    /// a pointer to the block of interest
    /// #### return
    /// true if the pointer is in between the start of page and the left most byte of the static sector.
    /// false otherwise. Blocks in the static sector CANNOT be detected with this function.
    pub fn block_is_in_space(&self, first_byte: *const u8) -> bool {
        self.start_of_page <= first_byte && first_byte < self.static_end
    }
    /// deletes a reserved block
    /// #### first_byte
    /// the first byte of the block
    /// #### return
    /// true if successful, false otherwise
    pub fn delete_block(&mut self, first_byte: *const u8) -> bool {
        #[cfg(feature = "condition")]
        {
            assert!(self.static_end > self.dynamic_end);
        }
        let (memory_block_size, code_block_start) =
            unsafe { code_block::read_from_right(first_byte.offset(-1)) };
        let code_block_start = code_block_start as *mut u8;
        let code_block_size = unsafe { code_block::get_block_size(code_block_start) };
        #[cfg(feature = "statistic")]
        {
            Statistic::freeDynamic(memory_block_size, first_byte);
        }
        if (code_block_start as usize + (2 * code_block_size) + memory_block_size)
            > self.static_end as usize
        {
            panic!("code block reaches into static space")
        }
        let mut left_neighbor = core::ptr::null_mut();
        let mut right_neighbor =
            (code_block_start as usize + (2 * code_block_size) + memory_block_size) as *mut u8;
        if right_neighbor as usize > self.static_end as usize {
            panic!("dynamic memory links into static space")
        }
        if self.start_of_page < code_block_start {
            left_neighbor = unsafe { get_left_neighbor(code_block_start.offset(-1)) as *mut u8 };
        }
        if !left_neighbor.is_null() && !code_block::is_free(left_neighbor) {
            left_neighbor = core::ptr::null_mut();
        }
        if !right_neighbor.is_null()
            && (right_neighbor as usize >= self.static_end as usize
                || !code_block::is_free(right_neighbor))
        {
            right_neighbor = core::ptr::null_mut();
        }
        unsafe { self.merge_free_space(left_neighbor, code_block_start, right_neighbor) };
        #[cfg(feature = "condition")]
        {
            unsafe {
                assert!(
                    (left_neighbor.is_null()
                        && self.bucket_list.is_in_list(code_block_start).0
                        && code_block::is_free(code_block_start))
                        || (self.bucket_list.is_in_list(left_neighbor).0
                            && code_block::is_free(left_neighbor)),
                )
            };
            assert!(self.static_end > self.dynamic_end);
        }
        return true;
    }
    /// #### return
    /// a pointer to the first byte in the page
    pub fn get_start_of_page(&self) -> *const u8 {
        self.start_of_page
    }
    /// #### return
    /// a pointer to the first byte in the static area
    pub fn get_static_end(&self) -> *const u8 {
        self.static_end
    }
    /// #### return
    /// the dynamic end
    pub fn get_dynamic_end(&self) -> *const u8 {
        self.dynamic_end
    }
    /// #### return
    /// the bucket list
    pub fn get_bucket_list(&self) -> &BucketList {
        &self.bucket_list
    }
    /// Merges up to three blocks into one Block of free Space.
    /// Only free blocks are merged.
    /// The bucket_list will be updated accordingly<br/>
    /// WARNING: the blocks have to be adjacent to each other. Merging distant blocks will cause undefined behavior.
    /// Probably causing the world as we know it, to cease to exist!
    /// #### left_block
    /// left_block to be merged. Ignored if null
    /// #### middle_block
    /// middle Block to be merged
    /// #### right_block
    /// right Block to be merged. Ignored if null
    /// #### return
    /// the new block of free space
    #[inline]
    unsafe fn merge_free_space(
        &mut self,
        left_block: *mut u8,
        middle_block: *mut u8,
        right_block: *mut u8,
    ) -> *const u8 {
        #[cfg(feature = "condition")]
        {
            assert!(!code_block::is_free(middle_block));
            assert!(right_block.is_null() || self.bucket_list.is_in_list(right_block).0);
            assert!(left_block.is_null() || self.bucket_list.is_in_list(left_block).0);
        }
        if left_block.is_null() {
            if !right_block.is_null() {
                self.bucket_list.delete_from_list(right_block);
                self.merge_with_right(middle_block, right_block);
            }
            code_block::set_free(middle_block, true);
            copy_code_block_to_end(middle_block, code_block::get_block_size(middle_block));
            self.bucket_list.add_to_list(middle_block);
            #[cfg(feature = "condition")]
            {
                assert!(code_block::is_free(middle_block));
                assert!(self.bucket_list.is_in_list(middle_block).0);
            }
            return middle_block;
        } else {
            if !right_block.is_null() {
                self.bucket_list.delete_from_list(right_block);
                self.merge_with_right(middle_block, right_block);
            }
            self.bucket_list.delete_from_list(left_block);

            self.merge_with_left(left_block, middle_block);
            code_block::set_free(left_block, true);
            copy_code_block_to_end(left_block, code_block::get_block_size(left_block));
            self.bucket_list.add_to_list(left_block);
            #[cfg(feature = "condition")]
            {
                assert!(code_block::is_free(left_block));
                assert!(self.bucket_list.is_in_list(left_block).0);
            }
            left_block
        }
    }
    /// Merges both blocks to one. The types of Blocks are ignored.
    #[inline]
    unsafe fn merge_with_left(&self, left_block: *mut u8, middle_block: *const u8) {
        #[cfg(feature = "condition")]
        {
            assert!(code_block::is_free(left_block));
        }
        let left_end = left_block;
        let right_end = get_right_most_end(middle_block);
        let (code_block_size, _) = code_block::get_code_block_for_internal_size(
            left_end,
            right_end as usize - left_end as usize + 1,
            true,
        );
        copy_code_block_to_end(left_end, code_block_size);
        #[cfg(feature = "condition")]
        {
            assert!(code_block::is_free(left_end));
            assert!(
                code_block::read_from_left(left_end)
                    == right_end as usize - left_end as usize - 2 * code_block_size + 1
            );
        }
    }
    //// Merges both blocks to one. The types of Blocks are ignored.
    #[inline]
    unsafe fn merge_with_right(&self, middle_block: *const u8, right_block: *const u8) {
        #[cfg(feature = "condition")]
        {
            assert!(code_block::is_free(right_block));
        }
        let left_end = middle_block as *mut u8;
        let right_end = get_right_most_end(right_block);
        let (code_block_size, _) = code_block::get_code_block_for_internal_size(
            left_end,
            right_end as usize - left_end as usize + 1,
            true,
        );
        copy_code_block_to_end(left_end, code_block_size);
        #[cfg(feature = "condition")]
        {
            assert!(code_block::is_free(middle_block));
            assert!(
                code_block::read_from_left(left_end)
                    == right_end as usize - left_end as usize - 2 * code_block_size + 1
            );
        }
    }
    /// Takes free space und cut the specified amount from space, starting at the left end. The new block has the adapted
    /// code blocks with the new size.
    /// #### free_space
    /// space to be cut
    /// #### bytesToCutOf
    /// amount of bytes to cut off from the left
    /// #### return
    /// null if the resulting block would be smaller than the smallest addressable block. A pointer to the
    /// resulting block otherwise
    #[inline]
    fn cut_left_from_free_space(&self, mut free_space: *mut u8, bytes_to_cut_of: usize) -> *mut u8 {
        #[cfg(feature = "condition")]
        {
            assert!(
                free_space >= self.start_of_page as *mut u8
                    && free_space < self.static_end as *mut u8
            );
            assert!(get_size(free_space) >= bytes_to_cut_of);
        }
        if (get_size(free_space) as usize - bytes_to_cut_of) < SMALLEST_POSSIBLE_FREE_SPACE {
            #[cfg(feature = "condition")]
            {}
            return core::ptr::null_mut();
        } else {
            free_space = unsafe {
                push_beginning_right(free_space, free_space.offset(bytes_to_cut_of as isize))
            };
            #[cfg(feature = "condition")]
            {
                unsafe {
                    assert!(
                        get_next(free_space, self.start_of_page).is_null()
                            || (get_next(free_space, self.start_of_page)
                                >= self.start_of_page as *mut u8
                                && get_next(free_space, self.start_of_page)
                                    < self.static_end as *mut u8),
                    )
                };
                assert!(get_size(free_space) >= 6);
            }
            return free_space;
        }
    }
    /// Takes free space und cut the specified amount from space, starting at the right end. The new block has the adapted
    /// code blocks with the new size.
    /// #### free_space
    /// space to be cut
    /// #### bytesToCutOf
    /// amount of bytes to cut off from the left
    /// #### return
    /// null if the resulting block would be smaller than the smallest addressable block. A pointer to the
    /// resulting block otherwise
    #[inline]
    fn cut_right_from_free_space(&self, free_space: *mut u8, bytes_to_cut_of: usize) -> *mut u8 {
        #[cfg(feature = "condition")]
        {
            assert!(get_size(free_space) >= bytes_to_cut_of); //there must be enough space in the freespace
            assert!(
                free_space >= self.start_of_page as *mut u8
                    && free_space < self.static_end as *mut u8
            );
            //the freespace must be in the page
        }
        if (get_size(free_space) - bytes_to_cut_of) < SMALLEST_POSSIBLE_FREE_SPACE {
            #[cfg(feature = "condition")]
            {
                //see if clause
            }
            return core::ptr::null_mut();
        } else {
            unsafe {
                push_end_left(
                    free_space,
                    get_right_most_end(free_space).offset(-(bytes_to_cut_of as isize)),
                )
            };
            #[cfg(feature = "condition")]
            {
                unsafe {
                    //the next pointer must either be the invalid pointer or must point into the page
                    assert!(
                        get_next(free_space, self.start_of_page).is_null()
                            || (get_next(free_space, self.start_of_page)
                                >= self.start_of_page as *mut u8
                                && get_next(free_space, self.start_of_page)
                                    < self.static_end as *mut u8),
                    )
                };
                assert!(free_space >= self.start_of_page as *mut u8); //freespace must still be in the page
                assert!(get_right_most_end(free_space) < self.static_end); //freespace may not go into the static area
            }
            free_space
        }
    }
    /// generates the first bucket entry
    /// #### return
    /// the first bucket entry
    #[inline]
    unsafe fn generate_first_bucket_entry(
        start_of_page: *mut u8,
        end_of_page: *const u8,
    ) -> *mut u8 {
        let free_space = start_of_page;
        let (code_block_size, _) = code_block::get_code_block_for_internal_size(
            start_of_page,
            end_of_page as usize - start_of_page as usize,
            true,
        );
        copy_code_block_to_end(free_space, code_block_size);
        set_next(free_space, core::ptr::null(), start_of_page);
        return free_space;
    }
}
