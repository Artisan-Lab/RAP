fn genvec() -> Vec<u8> {
    let mut s = String::from("a tmp string");
    let ptr = s.as_mut_ptr();
    let v;
    unsafe{
        v = Vec::from_raw_parts(
            ptr, s.len(), s.len());
    }
    return v;
}

fn main() {
    let v = genvec();
}