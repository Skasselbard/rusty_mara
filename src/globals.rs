pub const LAST_LINEAR_4_SCALING: usize = 32;
pub const LAST_LINEAR_16_SCALING: usize = 128;
pub const LARGEST_BUCKET_SIZE: usize = 1024;
pub const LOG2_1024: usize = 10;
pub const LOG2_128: usize = 7;
pub const BUCKET_LIST_SIZE: usize = LAST_LINEAR_4_SCALING / 4
    + (LAST_LINEAR_16_SCALING - LAST_LINEAR_4_SCALING + 4) / 16
    + (LOG2_1024 - LOG2_128)
    + 1;

pub const MAX_PAGE_SIZE: usize = 0x1000_0000_0000; //2^32 byte ~ 4Gb
pub const SMALLEST_POSSIBLE_FREE_SPACE: usize = 6; //6 byte

fn log2_64(x: u64) -> usize {
    if x == 0 {
        panic!("log2 is not defined for zero")
    }
    let mut x = x;
    let table: [u8; 64] = [
        63, 0, 58, 1, 59, 47, 53, 2, 60, 39, 48, 27, 54, 33, 42, 3, 61, 51, 37, 40, 49, 18, 28, 20,
        55, 30, 34, 11, 43, 14, 22, 4, 62, 57, 46, 52, 38, 26, 32, 41, 50, 36, 17, 19, 29, 10, 13,
        21, 56, 45, 25, 31, 35, 16, 9, 12, 44, 24, 15, 8, 23, 7, 6, 5,
    ];

    x |= x >> 1;
    x |= x >> 2;
    x |= x >> 4;
    x |= x >> 8;
    x |= x >> 16;
    x |= x >> 32;
    table[((x - (x >> 1)).wrapping_mul(0x07EDD5E59A4E28C2) >> 58) as usize] as usize
}

fn log2_32(x: u32) -> usize {
    if x == 0 {
        panic!("log2 is not defined for zero")
    }
    let mut x = x;
    let table: [u8; 32] = [
        0, 9, 1, 10, 13, 21, 2, 29, 11, 14, 16, 18, 22, 25, 3, 30, 8, 12, 20, 28, 15, 17, 24, 7,
        19, 27, 23, 6, 26, 5, 4, 31,
    ];

    x |= x >> 1;
    x |= x >> 2;
    x |= x >> 4;
    x |= x >> 8;
    x |= x >> 16;
    table[(x.wrapping_mul(0x07C4ACDD) >> 27) as usize] as usize
}

/// log is not defined in core so we must define it our selfs
/// https://stackoverflow.com/questions/11376288/fast-computing-of-log2-for-64-bit-integers#11398748
pub fn log2(x: usize) -> usize {
    #[cfg(feature = "consistency-checks")]
    {
        assert!(x > 0)
    }
    if cfg!(target_pointer_width = "64") {
        log2_64(x as u64)
    } else if cfg!(target_pointer_width = "32") {
        log2_32(x as u32)
    } else {
        panic! {"log2 is only independent on 32 bit and 64 bit pointer sizes. You can call the size specific version log2_64 or log2_32"}
    }
}

#[test]
pub fn test_log() {
    assert_eq!(log2_32(0xffff_ffff), 31);
    assert_eq!(log2_32(0x0001_0001), 16);
    assert_eq!(log2_32(0x0001_0000), 16);
    assert_eq!(log2_32(0x0000_ffff), 15);
    assert_eq!(log2_32(0x0000_0101), 8);
    assert_eq!(log2_32(0x0000_0100), 8);
    assert_eq!(log2_32(0x0000_00ff), 7);
    assert_eq!(log2_32(0x0000_0011), 4);
    assert_eq!(log2_32(0x0000_0010), 4);
    assert_eq!(log2_32(0x0000_000f), 3);
    assert_eq!(log2_32(0x0000_0005), 2);
    assert_eq!(log2_32(0x0000_0004), 2);
    assert_eq!(log2_32(0x0000_0003), 1);
    assert_eq!(log2_32(0x0000_0002), 1);
    assert_eq!(log2_32(0x0000_0001), 0);

    assert_eq!(log2_64(0xffff_ffff_ffff_ffff), 63);
    assert_eq!(log2_64(0x0000_0001_0000_0001), 32);
    assert_eq!(log2_64(0x0000_0001_0000_0000), 32);
    assert_eq!(log2_64(0x0000_0000_ffff_ffff), 31);
    assert_eq!(log2_64(0x0000_0000_0001_0001), 16);
    assert_eq!(log2_64(0x0000_0000_0001_0000), 16);
    assert_eq!(log2_64(0x0000_0000_0000_ffff), 15);
    assert_eq!(log2_64(0x0000_0000_0000_0101), 8);
    assert_eq!(log2_64(0x0000_0000_0000_0100), 8);
    assert_eq!(log2_64(0x0000_0000_0000_00ff), 7);
    assert_eq!(log2_64(0x0000_0000_0000_0011), 4);
    assert_eq!(log2_64(0x0000_0000_0000_0010), 4);
    assert_eq!(log2_64(0x0000_0000_0000_000f), 3);
    assert_eq!(log2_64(0x0000_0000_0000_0005), 2);
    assert_eq!(log2_64(0x0000_0000_0000_0004), 2);
    assert_eq!(log2_64(0x0000_0000_0000_0003), 1);
    assert_eq!(log2_64(0x0000_0000_0000_0002), 1);
    assert_eq!(log2_64(0x0000_0000_0000_0001), 0);

    assert_eq!(log2(0x10), 4);
}
