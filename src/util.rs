/// Match any characters that are not allowed in user input. Can be used to strip undesired unicode
/// symbols.
pub fn disallowed_input_chars(c: char) -> bool {
    !char::is_alphanumeric(c) && !char::is_ascii_punctuation(&c) && !char::is_ascii_whitespace(&c)
}

pub fn split_args(args_str: &str) -> Vec<String> {
    args_str
        .replace(disallowed_input_chars, "")
        .split_whitespace()
        .map(ToString::to_string)
        .collect()
}