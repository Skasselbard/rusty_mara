#![cfg(feature = "consistency_tests")]

extern crate rusty_mara;
#[test]
fn test_short() {
    use rusty_mara::TestBuilder;
    const MEMORY_SIZE: usize = 0x4000_0000; // 1GB
    let mut memory: Box<[u8]> = vec![0; MEMORY_SIZE].into_boxed_slice();
    let mut standard = TestBuilder::new((*memory).as_mut_ptr(), MEMORY_SIZE).build();
    standard.run();
}

#[test]
fn test_long() {
    use rusty_mara::TestBuilder;
    const MEMORY_SIZE: usize = 0x4000_0000; // 1GB
    let mut memory: Box<[u8]> = vec![0; MEMORY_SIZE].into_boxed_slice();
    let mut test = TestBuilder::new((*memory).as_mut_ptr(), MEMORY_SIZE)
        .amount_new_variables(1_000_000)
        .build();
    test.run();
}
