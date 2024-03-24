fn main() {
    let buf = Box::new("buffer");
    let ptr = Box::into_raw(buf);
}