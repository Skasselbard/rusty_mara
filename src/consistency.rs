#![cfg(feature = "consistency_tests")]

use crate::code_block;
use crate::page::Page;
use crate::space::Space;
use crate::{AllocationData, Mara};
use core::mem::size_of;
use rand::distributions::{
    uniform::{UniformFloat, UniformSampler},
    Uniform,
};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

///-1 = don't fill at all
/// 0 = fill with zeros
/// 1 = fill with ones
/// 2 = fill with the address of the start of the memory array
#[derive(PartialEq, Debug, Eq, Copy, Clone)]
pub enum FillRequestedMemory {
    NoFill,
    Zeroes,
    Ones,
    AddressShortcut,
}

pub struct Test {
    /// Number of FreeSpaces which can not be found in the BucketList ( => memory leaks)
    free_space_not_in_bucket_list: usize,
    /// Number of blocks, which do not contain the original content.
    corrupted_blocks: usize,
    /// what should be done with the allocated data
    fill_strategy: FillRequestedMemory,

    amount_new_variables: usize,
    /// dynamic allocation propability.
    /// If no dynamic block is allocated a static block is allocted instead
    p_free: f64,
    /// minimal allocation size per block
    min_size: usize,
    /// maximal allocation size per block
    max_size: usize,
    /// after an iteration all pages are deleted
    max_iterations: usize,
    /// the seed that is used for the rng
    seed: usize,

    mara: Mara,
}

pub struct TestBuilder {
    /// what should be done with the allocated data
    fill_strategy: FillRequestedMemory,

    amount_new_variables: usize,
    /// probability to free a dynamic block
    p_free: f64,
    /// minimal allocation size per block
    min_size: usize,
    /// currently ignored
    average_size: usize,
    /// maximal allocation size per block
    max_size: usize,
    /// after an iteration all pages are deleted
    max_iterations: usize,
    /// the seed that is used for the rng
    seed: usize,

    memory: *mut u8,
    memory_size: usize,
}

impl TestBuilder {
    pub fn new(memory: *mut u8, memory_size: usize) -> Self {
        Self {
            fill_strategy: FillRequestedMemory::NoFill,
            max_iterations: 1,
            amount_new_variables: 50000,
            p_free: 0.5,
            min_size: 4,
            average_size: 16,
            max_size: 1000,
            seed: 123456789,
            memory,
            memory_size,
        }
    }

    pub fn build(self) -> Test {
        let mara = Mara::new(self.memory, self.memory_size);
        Test {
            free_space_not_in_bucket_list: 0,
            corrupted_blocks: 0,
            fill_strategy: self.fill_strategy,
            max_iterations: self.max_iterations,
            amount_new_variables: self.amount_new_variables,
            p_free: self.p_free,
            min_size: self.min_size,
            max_size: self.max_size,
            seed: self.seed,
            mara,
        }
    }

    pub fn fill_strategy(mut self, strategy: FillRequestedMemory) -> Self {
        self.fill_strategy = strategy;
        self
    }

    pub fn amount_new_variables(mut self, amount: usize) -> Self {
        self.amount_new_variables = amount;
        self
    }
    pub fn p_free(mut self, propability: f64) -> Self {
        self.p_free = propability;
        self
    }
    pub fn min_size(mut self, block_size: usize) -> Self {
        self.min_size = block_size;
        self
    }
    pub fn average_size(mut self, block_size: usize) -> Self {
        self.average_size = block_size;
        self
    }
    pub fn max_size(mut self, block_size: usize) -> Self {
        self.max_size = block_size;
        self
    }
    pub fn max_iterations(mut self, count: usize) -> Self {
        self.max_iterations = count;
        self
    }
    pub fn seed(mut self, seed: usize) -> Self {
        self.seed = seed;
        self
    }
}

impl Test {
    /// Runs a test with the arguments given. Customizable parameters are amount of requests, probability to request
    /// dynamic memory, probability to free a dynamic block after each (dynamic AND static) request, size information
    /// about the requested blocks (min, max), iterations (after each iteration, all
    /// pages are deleted and another n requests are made), seed for the RNG.
    /// After a request, the received block is filled with values depending on FILL_REQUESTED_MEMORY. Before freeing a
    /// block, it is filled with zeroes to make debugging easier.
    /// The time for each run is measured. After a run, a consistency check is performed.
    /// After completion, prints information about the test in the following order:
    /// type seed time dynamicMemoryPeak dynamicBlocksPeak staticMemoryPeak staticBlockPeak corrupted_blocks freeSpaceNotInBL
    pub fn run(&mut self) {
        let begin;
        let mut dynamic_pointers: Vec<Space>;
        if cfg!(no_std) {
            unimplemented!()
        } else {
            use std::time::Instant;
            begin = Instant::now();
            dynamic_pointers = Vec::new();
        }

        let mut rng = SmallRng::seed_from_u64(self.seed as u64);
        let dynamic_variable_distribution = Uniform::new(0, self.amount_new_variables - 1);
        let probability_distribution: UniformFloat<f64> = UniformFloat::new(0.0, 1.0);
        let size_distribution = Uniform::new(self.min_size, self.max_size);

        println!("seed\tseconds");
        for _iterations in 0..=self.max_iterations {
            for _v in 0..=self.amount_new_variables {
                let mut var_size;
                loop {
                    // generate a random size in the given boundaries
                    var_size = rng.sample(size_distribution);
                    var_size = var_size + (4 - (var_size % 4));
                    if !(var_size < self.min_size || var_size > self.max_size) {
                        break;
                    }
                }

                let mut space = Space::new();
                // request address to dynamic memory and save the address for later deletion
                space.set_ptr(self.mara.dynamic_new(var_size));
                space.set_size(var_size);
                dynamic_pointers.push(space);
                if self.fill_strategy != FillRequestedMemory::NoFill {
                    // write address to address
                    self.write_space(&mut space);
                }
                // self.check_page();
                // maybe free a dynamic variable
                let rnd_val: f64 = probability_distribution.sample(&mut rng);
                if !dynamic_pointers.is_empty() && rnd_val <= self.p_free {
                    let deleted_index =
                        rng.sample(dynamic_variable_distribution) % dynamic_pointers.len() as usize;
                    let to_delete = *dynamic_pointers.get(deleted_index).expect("item not found");
                    unsafe {
                        for i in 0..to_delete.size() {
                            *(to_delete.ptr().add(i)) = 0b0000_0000;
                        }
                    }

                    self.mara.dynamic_delete(to_delete.ptr());
                    dynamic_pointers.remove(deleted_index);
                    // self.check_page();
                }
            }
            self.check_page();
            let elapsed = begin.elapsed();
            println!("{}\t{}", self.seed, elapsed.as_secs(),);
        }
    }

    /**
     * Writes values into the block at the given address of the given size. Content depends on the of the
     * FILL_REQUESTED_MEMORY macro. If the block is to be filled with its starting address, any rest of 7 bytes or
     * smaller is ignored.
     *
     * \param address the block's starting address
     * \param size the block's size
     */

    fn write_space(&self, space: &mut Space) {
        if self.fill_strategy != FillRequestedMemory::NoFill {
            let value_at_address = match self.fill_strategy {
                FillRequestedMemory::NoFill => panic!("should be unreachable"),
                FillRequestedMemory::Zeroes => 0,
                FillRequestedMemory::Ones => 1,
                FillRequestedMemory::AddressShortcut => space.ptr() as usize,
            };
            // iterate in windows of usize
            for i in 0..space.size() / size_of::<usize>() {
                unsafe { *(space.ptr() as *mut usize).add(i) = value_at_address };
            }
        }
    }
    /// Checks the pages for consistency by iterating over them. At each page, reads the first block's size and status
    /// from the CodeBlock at the start of the page.
    /// If the block is free, tries to find it in the BucketList of the page to ensure that it is still allocatable.
    /// If the block is not free, checks it for consistency by checking if the content corresponds to what write_into_block
    /// had written into it after requesting it.
    /// Errors are counted using the variables "free_space_not_in_bucket_list" and "corrupted_blocks".
    fn check_page(&mut self) {
        unsafe {
            self.free_space_not_in_bucket_list = 0;
            self.corrupted_blocks = 0;

            let page = self.mara.page_list().get_page();
            let mut alloc = AllocationData::new();
            alloc.set_page(page as *mut Page);
            alloc.set_data_start((*page).start_of_page() as *mut u8);
            alloc.cache_code_blocks();
            loop {
                alloc.check_consistency();
                if !code_block::is_free(alloc.data_start()) {
                    (*page).bucket_list().check_in_list(&alloc.space, false);
                    if self.fill_strategy != FillRequestedMemory::NoFill {
                        for i in 0..(alloc.space.size() / size_of::<usize>()) {
                            let valid = match self.fill_strategy {
                                FillRequestedMemory::NoFill => panic!("should be unreachable"),
                                FillRequestedMemory::Zeroes => {
                                    *((alloc.data_start() as *mut usize).add(i)) == 0
                                }
                                FillRequestedMemory::Ones => {
                                    *((alloc.data_start() as *mut usize).add(i)) == 1
                                }
                                FillRequestedMemory::AddressShortcut => {
                                    *((alloc.data_start() as *mut usize).add(i))
                                        == *(alloc.data_start() as *mut usize)
                                }
                            };
                            if !valid {
                                panic!("corupted allocation")
                            }
                        }
                    }
                } else {
                    (*page).bucket_list().check_in_list(&alloc.space, true);
                }
                if let Some(neighbor) = alloc.right_neighbor() {
                    alloc = neighbor;
                } else {
                    assert_eq!(alloc.data_end(), (*page).end_of_page() as *mut u8);
                    break;
                }
            }
        }
    }
}
