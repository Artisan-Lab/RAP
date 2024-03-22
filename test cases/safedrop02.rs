struct Foo{
    vec : Vec<i32>,
}

impl Foo {
    pub unsafe fn read_from(src: &mut Read) -> Foo{
        let mut foo = mem::uninitialized::<Foo>();
        let s = slice::from_raw_parts_mut(
            &mut foo as *mut _ as *mut u8,
            mem::size_of::<Foo>());
        src.read_exact(s);
        foo
    }
}