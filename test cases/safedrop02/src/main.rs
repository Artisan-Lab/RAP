#[derive(Default)]
struct Foo {
    vec : Vec<i32>,
}

impl Foo {
    pub unsafe fn read_from(src: &mut dyn std::io::Read) -> Foo {
        let foo = core::mem::MaybeUninit::new(Foo::default());
        let mut foo = core::mem::MaybeUninit::assume_init(foo);
        let s = core::slice::from_raw_parts_mut(
            &mut foo as *mut _ as *mut u8,
            core::mem::size_of::<Foo>());
        src.read_exact(s);
        foo
    }
}

fn main() {

}