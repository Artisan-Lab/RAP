#![feature(fn_traits)]
use std::ptr::drop_in_place;
// ptr::drop_in_place
fn main() {
    let mut x = Box::from(0);
    let ptr = &mut x as *mut Box<i32>;
    drop(x);
    unsafe {
        drop_in_place(ptr);
    }
    let numbers = vec![10, 20, 30, 40, 50];
    let mut iter = numbers.iter();
    println!("{:?}", iter.next()); 

    let mut call_count = 0;
    let mut c = || { call_count += 1; };
    c.call_mut(());
}
