use crate::bucket_list::BucketList;
use crate::code_block;
use crate::free_space::*;
use crate::page::Page;
use crate::{AllocationData, Mara};
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

    page_size: usize,
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
            page_size: 0x80_0000, // 100 kb
            memory,
            memory_size,
        }
    }

    pub fn build(self) -> Test {
        let mara = Mara::new(self.page_size, self.memory, self.memory_size);
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
        let mut dynamic_pointers: Vec<*mut usize>;
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

        println!("seed\tseconds\tcorrupted blocks\tbucket list free space");
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

                let address: *mut usize;
                // request address to dynamic memory and save the address for later deletion
                address = self.mara.dynamic_new(var_size) as *mut usize;
                dynamic_pointers.push(address);
                if self.fill_strategy != FillRequestedMemory::NoFill {
                    // write address to address
                    Self::write_into_block(address, var_size, self.fill_strategy);
                }
                // maybe free a dynamic variable
                let rnd_val: f64 = probability_distribution.sample(&mut rng);
                if !dynamic_pointers.is_empty() && rnd_val <= self.p_free {
                    let deleted_index =
                        rng.sample(dynamic_variable_distribution) % dynamic_pointers.len() as usize;
                    let to_delete =
                        *dynamic_pointers.get(deleted_index).expect("item not found") as *mut usize;
                    unsafe {
                        let size = code_block::read_from_left(to_delete as *mut u8);
                        for i in 0..size {
                            *(to_delete.add(i)) = 0b00000000;
                        }
                    }

                    self.mara.dynamic_delete(to_delete as *mut u8);
                    dynamic_pointers.remove(deleted_index);
                }
            }

            let elapsed = begin.elapsed();

            self.check_pages();
            println!(
                "{}\t{}\t{}\t\t\t{}",
                self.seed,
                elapsed.as_secs(),
                self.corrupted_blocks,
                self.free_space_not_in_bucket_list
            );
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

    fn write_into_block(address: *mut usize, size: usize, fill_option: FillRequestedMemory) {
        if fill_option != FillRequestedMemory::NoFill {
            let value_at_address = match fill_option {
                FillRequestedMemory::NoFill => panic!("should be unreachable"),
                FillRequestedMemory::Zeroes => 0,
                FillRequestedMemory::Ones => 1,
                FillRequestedMemory::AddressShortcut => address as usize,
            };
            for i in 0..size {
                unsafe { *(address.offset(i as isize)) = value_at_address };
            }
        }
    }
    /// Checks the pages for consistency by iterating over them. At each page, reads the first block's size and status
    /// from the CodeBlock at the start of the page.
    /// If the block is free, tries to find it in the BucketList of the page to ensure that it is still allocatable.
    /// If the block is not free, checks it for consistency by checking if the content corresponds to what write_into_block
    /// had written into it after requesting it.
    /// Errors are counted using the variables "free_space_not_in_bucket_list" and "corrupted_blocks".
    fn check_pages(&mut self) {
        unsafe {
            self.free_space_not_in_bucket_list = 0;
            self.corrupted_blocks = 0;

            let mut page = self.mara.page_list().get_first_page();
            loop {
                let bucket_list = (*page).bucket_list();
                let mut block_pointer = (*page).start_of_page();
                while block_pointer < (*page).end_of_page() {
                    let memory_size = code_block::read_from_left(block_pointer);
                    let code_block_size = code_block::get_block_size(block_pointer);
                    if !code_block::is_free(block_pointer) {
                        if self.fill_strategy != FillRequestedMemory::NoFill {
                            let memory_start =
                                (block_pointer as usize + code_block_size) as *mut usize;
                            for i in 0..(memory_size / 8) {
                                let valid = match self.fill_strategy {
                                    FillRequestedMemory::NoFill => panic!("should be unreachable"),
                                    FillRequestedMemory::Zeroes => *(memory_start.add(i)) == 0,
                                    FillRequestedMemory::Ones => *(memory_start.add(i)) == 0,
                                    FillRequestedMemory::AddressShortcut => {
                                        *(memory_start.add(i)) == *memory_start
                                    }
                                };
                                if !valid {
                                    self.corrupted_blocks = self.corrupted_blocks + 1;
                                }
                            }
                        }
                    } else {
                        let bl_index = BucketList::lookup_bucket(memory_size);
                        let mut current_element = AllocationData::new();
                        current_element
                            .set_data_start(bucket_list.get_from_bucket_list(bl_index) as *mut u8);
                        current_element.set_page(page as *mut Page);
                        while current_element.data_start() != block_pointer as *mut u8 {
                            if current_element.data_start().is_null() {
                                break;
                            }
                            current_element.set_data_start(get_next(&current_element) as *mut u8)
                        }
                        if current_element.data_start().is_null() {
                            self.free_space_not_in_bucket_list =
                                self.free_space_not_in_bucket_list + 1;
                        }
                    }
                    block_pointer =
                        block_pointer.offset((memory_size + 2 * code_block_size) as isize);
                }
                page = (*page).get_next_page();
                if !(page != (*self.mara.page_list()).get_first_page()) {
                    break;
                }
            }
        }
    }
}
