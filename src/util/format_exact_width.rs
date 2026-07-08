pub fn pad_left<D: std::fmt::Display>(s: D, len: usize) -> String {
    let mut result = format!("{s}");
    if result.len() < len {
        let spaces = ' '.to_string().repeat(len - result.len());
        result = spaces + &result;
    }
    result
}

pub fn pad_right<D: std::fmt::Display>(s: D, len: usize) -> String {
    let mut result = format!("{s}");
    if result.len() < len {
        let spaces = ' '.to_string().repeat(len - result.len());
        result = result + &spaces;
    }
    result
}
