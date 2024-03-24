use crate::rap_info;

#[derive(Default)]
pub struct HelloWorld  {}


impl HelloWorld {
    pub fn new() -> Self { HelloWorld::default() }

    pub fn start(&self) { rap_info!("Hello World from RAP frontend!!!!!"); }
}