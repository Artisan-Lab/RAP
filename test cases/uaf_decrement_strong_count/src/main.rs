#![feature(allocator_api)]
use std::rc::Rc;

// decrement_strong_count
fn main() {
    let five = Rc::new(5);

    unsafe {
        assert_eq!(1, Rc::strong_count(&five));
        let ptr = Rc::into_raw(five);
        Rc::decrement_strong_count(ptr);
        println!("{}", *ptr);
    }
}

// decrement_strong_count_in
// use std::alloc::System;
// fn main() {
//     let five = Rc::new_in(5, System);

//     unsafe {
//         assert_eq!(1, Rc::strong_count(&five));
//         let ptr = Rc::into_raw(five);
//         Rc::decrement_strong_count_in(ptr, System);
//         println!("{}", *ptr);
//     }
// }
