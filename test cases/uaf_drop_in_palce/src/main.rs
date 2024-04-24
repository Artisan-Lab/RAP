use std::ptr::drop_in_place;
// ptr::drop_in_place
fn main() {
    let mut x = Box::from(0);
    let ptr = &mut x as *mut Box<i32>;
    drop(x);
    unsafe {
        drop_in_place(ptr);
    }
}

// primitive.pointer
// fn main() {
//     let mut x = Box::from(0);
//     let ptr = &mut x as *mut Box<i32>;
//     drop(x);
//     unsafe {
//         ptr.drop_in_place();
//     }
// }

// NonNull
// fn main() {
//     let mut x = Box::from(0);
//     let ptr = NonNull::new(&mut x as *mut Box<i32>).expect("ptr is null!");
//     drop(x);
//     unsafe {
//         ptr.drop_in_place();
//     }
// }
