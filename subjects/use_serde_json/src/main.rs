fn main() { let v: serde_json::Value = serde_json::from_str("{\"a\":1}").unwrap(); println!("{}", v); }
