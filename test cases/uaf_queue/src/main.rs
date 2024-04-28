use queue::Queue;


mod queue {
    pub struct Queue<T> {
        qdata: Vec<T>,
    }

    impl<T> Queue<T>
    where
        T: std::fmt::Display,
        T: Clone,
    {
        // Create a Queue
        pub fn new() -> Self {
            Queue { qdata: Vec::new() }
        }

        // Add item to the Queue
        pub fn push(&mut self, item: T) {
            self.qdata.push(item);
        }

        // Pop the top i item from the Queue
        // And free the pointer
        pub fn pop(&self, i: usize) {
            let l = self.qdata.len();
            if l > i {
                for _ in 0..i + 1 {
                    let raw = &self.qdata as *const Vec<T> as *mut Vec<T>;
                    unsafe { (*raw).remove(0); }
                }
            }
        }

        // Get item from Queue and get pointer
        pub fn peek(&self) -> Option<&T> {
            if !self.qdata.is_empty() {
                let raw = &self.qdata[0] as *const T as *mut T;
                unsafe { Some(& *raw) }
            } else {
                None
            }
        }
    }
}


fn main() {
    let mut q: Queue<String> = Queue::new();
    q.push(String::from("hello"));
    let e = q.peek().unwrap();
    q.pop(0);
    println!("{}", *e);
}
