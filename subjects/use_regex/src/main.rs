fn main() { let re = regex::Regex::new(r"\d+").unwrap(); println!("{}", re.is_match("abc123")); }
