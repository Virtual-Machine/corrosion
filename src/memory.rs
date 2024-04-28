// Collection of helpers pertaining to memory manipulations

pub const fn align_val(val: usize, order: usize) -> usize {
    let o = (1usize << order) - 1;
    (val + o) & !o
}

pub unsafe fn memcpy(dest: *mut u8, src: *const u8, bytes: usize) {
    let bytes_as_8 = bytes / 8;
    let dest_as_8 = dest as *mut u64;
    let src_as_8 = src as *const u64;

    for i in 0..bytes_as_8 {
        *(dest_as_8.add(i)) = *(src_as_8.add(i));
    }
    let bytes_completed = bytes_as_8 * 8;
    let bytes_remaining = bytes - bytes_completed;
    for i in bytes_completed..bytes_remaining {
        *(dest.add(i)) = *(src.add(i));
    }
}
