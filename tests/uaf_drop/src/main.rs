use std::mem::ManuallyDrop;

fn main() {
    let mut slot = ManuallyDrop::<Box<u8>>::new(Box::new(1));
    unsafe {
        ManuallyDrop::drop(&mut slot);
    }
    println!("{:?}", slot);
}
