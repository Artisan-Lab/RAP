use std::slice;

fn test1() {
    let data: *const u8 = Box::leak(Box::new(0));
    let len: usize = (isize::MAX as usize) / std::mem::size_of::<u8>() + 1;
    // 'len' is out of the max value and causes a 'bounded' UB
    let slice: &[u8] = unsafe { slice::from_raw_parts(data, len) };
    if let Some(last_element) = slice.last() {
        println!("Last element: {}", last_element);
    } else {
        println!("Slice is empty");
    }
}

fn main() {
    test1();
}