/*
 *This is a benign case.
 */
use std::io::{Read, Result};

#[derive(Default)]
struct Foo {
    _vec: Vec<i32>,
}

impl Foo {
    pub unsafe fn read_from(src: &mut dyn Read) -> Result<Foo> {
        let mut foo = Foo::default();
        let s = core::slice::from_raw_parts_mut(
            &mut foo as *mut _ as *mut u8,
            core::mem::size_of::<Foo>(),);
        src.read_exact(s)?;
        Ok(foo)
    }
}

fn main() {
    match unsafe { Foo::read_from(&mut std::io::stdin()) } {
        Ok(_) => println!("Read successfully!"),
        Err(e) => eprintln!("Error: {}", e),
    }
}
