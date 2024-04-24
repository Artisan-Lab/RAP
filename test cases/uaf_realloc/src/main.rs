use std::alloc::{System, GlobalAlloc, Layout};

fn main() {
    let layout = unsafe { Layout::from_size_align_unchecked(1, 1) };
    unsafe {
        let ptr1 = System.alloc(layout) as *mut i8;
        ptr1.write(1);
        let ptr2 = System.realloc(ptr1 as *mut u8, layout, 2) as *mut i16;
        ptr2.write(2);
        println!("{}", *ptr2);
        System.dealloc(ptr1 as *mut u8, layout);
        // System.dealloc(ptr2 as *mut u8, layout);
    }
}
