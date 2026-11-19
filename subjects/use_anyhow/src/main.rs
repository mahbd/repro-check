fn f() -> anyhow::Result<u32> { Ok("7".parse::<u32>()?) } fn main() { println!("{}", f().unwrap()); }
