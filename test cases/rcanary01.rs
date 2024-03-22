fn main() {
    let mut buf = Box::new("buffer");
    let ptr = Box::into_raw(buf);
}