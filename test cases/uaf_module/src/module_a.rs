pub fn drop_vec_ptr<T>(ptr: *mut Vec<T>) {
    // 使用 unsafe 块调用 drop_in_place 函数释放 Vec 的内存
    unsafe {
        std::ptr::drop_in_place(ptr);
    }
}
