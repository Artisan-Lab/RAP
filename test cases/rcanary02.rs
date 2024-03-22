struct Proxy<T> {
    ptr: *mut T,
}
fn main() {
    let mut buf = Box::new("buffer");
    let ptr = Box::into_raw(buf);
    let proxy = Proxy { ptr };
}