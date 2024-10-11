enum Selector {
    First,
    Second,
}

fn foo<'a>(x: &'a i32, y: &'a i32, choice: Selector) -> &'a i32 {
    let a = match choice {
        Selector::First => x, 
        Selector::Second => y,
    };
    match choice {
        Selector::First => a, 
        Selector::Second => x,
    }
}

fn main() {
    let a = Box::new(10);
    let b = Box::new(20);
    let _result = foo(&a, &b, Selector::First);
    let _result = foo(&a, &b, Selector::Second);
}

