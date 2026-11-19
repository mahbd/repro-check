use clap::Parser; #[derive(Parser)] struct A { #[arg(long, default_value_t = 1)] n: u32 } fn main() { let a = A::parse(); println!("{}", a.n); }
