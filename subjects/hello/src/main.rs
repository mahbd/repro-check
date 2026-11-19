fn main() { let v: Vec<i32> = std::env::args().skip(1).map(|a| a.parse().unwrap()).collect(); println!("{:?}", v); }
