/// Match any characters that are not allowed in user input. Can be used to strip undesired unicode
/// symbols.
pub fn disallowed_input_chars(c: char) -> bool {
    !char::is_alphanumeric(c) && !char::is_ascii_punctuation(&c) && !char::is_ascii_whitespace(&c)
}
