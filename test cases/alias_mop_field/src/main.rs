struct Point {
    x: i32,
    y: i32,
}

fn foo(p1: &Point) -> &i32 {
    &p1.y
}

fn main() {
    let p = Box::new(Point { x:10, y:20 });
    let _r = foo(&p);
}

