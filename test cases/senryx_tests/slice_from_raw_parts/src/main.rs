use std::slice;

fn test1() {
    let data: *const u8 = Box::leak(Box::new(0));
    let len: usize = (isize::MAX as usize) / std::mem::size_of::<u8>() + 1;
    // Pass(Allocated \ Aligned):   data is allocated and aligned
    // Fail(Bounded): 'len' is out of the max value
    // Fail(Dereferencable \ Initialized): 'data' onnly points to the memory with a 'u8' size, but the 'len' is out of this range
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