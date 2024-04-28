use crate::alloc::{alloc_bytes, free_bytes};
use crate::memory::memcpy;
use crate::minixfs3::BLOCK_SIZE;
use crate::{print, println};
use core::{
    ops::{Index, IndexMut},
    ptr::null_mut,
};

// Buffer memory collection module

pub struct Buffer {
    buffer: *mut u8,
    len: usize,
}

impl Buffer {
    pub fn new(sz: usize) -> Self {
        Self {
            buffer: alloc_bytes(sz),
            len: sz,
        }
    }

    pub fn get_mut(&mut self) -> *mut u8 {
        self.buffer
    }

    pub fn get(&self) -> *const u8 {
        self.buffer
    }

    pub fn len(&self) -> usize {
        self.len
    }

    #[allow(dead_code)]
    pub fn print(&self) {
        println!("len: {}", self.len);
        for i in 0..self.len {
            print!("{}", unsafe { self.buffer.add(i).read() } as char);
        }
        println!();
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new(BLOCK_SIZE as usize)
    }
}

impl Index<usize> for Buffer {
    type Output = u8;
    fn index(&self, idx: usize) -> &Self::Output {
        unsafe { self.get().add(idx).as_ref().unwrap() }
    }
}

impl IndexMut<usize> for Buffer {
    fn index_mut(&mut self, idx: usize) -> &mut Self::Output {
        unsafe { self.get_mut().add(idx).as_mut().unwrap() }
    }
}

impl Clone for Buffer {
    fn clone(&self) -> Self {
        let mut new = Self {
            buffer: alloc_bytes(self.len()),
            len: self.len(),
        };
        unsafe {
            memcpy(new.get_mut(), self.get(), self.len());
        }
        new
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        if !self.buffer.is_null() {
            free_bytes(self.buffer);
            self.buffer = null_mut();
        }
    }
}
