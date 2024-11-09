use std::ptr;
use std::slice;
use std::mem::MaybeUninit;

struct MySliceWrapperTest<T> {
    data: *const T, 
    len: usize,     
}

impl<T> MySliceWrapperTest<T> {
    fn new() -> Self {
        let uninit_data = MaybeUninit::<[T; 10]>::uninit();
        let data = uninit_data.as_ptr() as *const T;
        MySliceWrapperTest { data, len: 10 }
    }

    fn get_slice(&self, offset: usize, length: usize) -> &[T] {
        assert!(offset + length <= self.len, "Requested slice is out of bounds");
        let adjusted_data = unsafe { self.data.add(offset) };
        // Fail(Allocated): 'adjusted_data' points to uninit memory
        // Fail(Aligned): 'adjusted_data' may be not aligned due to the offset
        unsafe { slice::from_raw_parts(adjusted_data, length) }
    }
}


fn test1() {
    let len: usize = 0;
    let data = ptr::null::<i32>();
    // Fail(Allocated): 'data' is null, which violates the requirement that it must be non-null
    let slice: &[i32] = unsafe { slice::from_raw_parts(data, len) };
}

fn test2() {
    let len: usize = 3;
    let uninit_data = MaybeUninit::<[i32; 3]>::uninit();
    let data = uninit_data.as_ptr() as *const i32;
    // Fail(Initialized): 'data' points to uninitialized memory, which violates the initialization requirement
    let slice: &[i32] = unsafe { slice::from_raw_parts(data, len) };
    println!("First element: {}", slice[0]);
}

fn test3() {
    let part1 = Box::new(1);
    let part2 = Box::new(2);
    let data = [Box::into_raw(part1), Box::into_raw(part2)].as_ptr() as *const i32;
    let len = 2;
    // Fail(Dereferencable): 'data' points across multiple allocated objects, violating the single allocation constraint
    let slice: &[i32] = unsafe { slice::from_raw_parts(data, len) };
    println!("Slice elements: {:?}", slice);
}

fn test4() {
    let unaligned = [0u8; 5];
    let data = unaligned.as_ptr().wrapping_offset(1) as *const i32;
    let len = 1;
    // Fail(Layout): 'data' is not aligned, violating the alignment requirement
    let slice: &[i32] = unsafe { slice::from_raw_parts(data, len) };
    println!("Slice elements: {:?}", slice);
}

fn test5() {
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

fn test6(a: &mut [u8], b: &[u32; 20]) {
    unsafe {
        let c = slice::from_raw_parts_mut(a.as_mut_ptr() as *mut u32, 20);
        for i in 0..20 {
            c[i] ^= b[i];
        }
    }
}

fn main() {
    test1();
    let mut x = [0u8;40];
    let y = [0u32;20];
    // test2(&mut x[1..32], &y);
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test1_allocated_non_null_data() {
        let len = 0;
        let data = ptr::null::<i32>();
        
        {
            unsafe { slice::from_raw_parts(data, len) };
        }
    }

    #[test]
    fn test2_initialized_consecutive_values() {
        // data属性 - Initialized: data必须指向已初始化的值
        let len = 3;
        let uninit_data = MaybeUninit::<[i32; 3]>::uninit();
        let data = uninit_data.as_ptr() as *const i32;

        // 此处期望未初始化的值会导致未定义行为
        #[should_panic]
        {
            unsafe { slice::from_raw_parts(data, len) };
        }
    }

    #[test]
    fn test3_dereferencable_memory_range() {
        // data属性 - Dereferencable: 内存范围应在单一分配的对象内
        // 通过手动分配多个内存对象以模拟不符合的情况
        let part1 = Box::new(1);
        let part2 = Box::new(2);
        let data = [Box::into_raw(part1), Box::into_raw(part2)].as_ptr() as *const i32;
        
        #[should_panic]
        {
            unsafe { slice::from_raw_parts(data, 2) };
        }
    }

    #[test]
    fn test4_layout_alignment() {
        // data属性 - Layout: 数据指针必须对齐
        let unaligned = [0u8; 5];
        let data = unaligned.as_ptr().wrapping_offset(1) as *const i32;
        
        #[should_panic]
        {
            unsafe { slice::from_raw_parts(data, 1) };
        }
    }

    #[test]
    fn test5_len_bounded() {
        // len属性 - Bounded: 切片大小不得超过isize::MAX
        let len = (isize::MAX as usize / std::mem::size_of::<i32>()) + 1;
        let data = ptr::null::<i32>();

        #[should_panic]
        {
            unsafe { slice::from_raw_parts(data, len) };
        }
    }
}
