/*
 * This is a buggy case: create_vec() returns a dangling pointer 
 */

use std::env;
mod module_a;

fn foo() {
    let mut v = vec![1, 2, 3, 4, 5];
    let raw_ptr = &mut v as *mut Vec<i32>;
    module_a::drop_vec_ptr(raw_ptr);
    println!("{:?}", v);
}

fn main() { 
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        foo();
    }
}
