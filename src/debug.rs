use crate::alloc;
use crate::minixfs3;
use crate::uart;

// Collection of helpers to aid the debugging process

#[allow(dead_code)]
pub fn heap() {
    alloc::debug_heap();
}

#[allow(dead_code)]
pub fn fs_cache() {
    minixfs3::debug_cache();
}

#[allow(dead_code)]
pub fn fs() {
    minixfs3::debug_fs();
}

#[allow(dead_code)]
pub fn dbg(text: &str) {
    uart::serial_debug(text);
}

#[allow(dead_code)]
pub fn number(label: &str, number: usize) {
    uart::serial_debug_number(label, number);
}

#[allow(dead_code)]
pub fn text(label: &str, text: &str) {
    uart::serial_debug_text(label, text);
}
