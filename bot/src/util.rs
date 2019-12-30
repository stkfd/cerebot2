use std::cmp::min;

use persistence::commands::attributes::InsertCommandAttributes;

use crate::handlers::error::CommandError;
use crate::state::BotContext;
use crate::Result;

/// Match any characters that are not allowed in user input. Can be used to strip undesired unicode
/// symbols.
pub fn disallowed_input_chars(c: char) -> bool {
    !char::is_alphanumeric(c)
        && !char::is_ascii_punctuation(&c)
        && !char::is_ascii_whitespace(&c)
        && !is_quote(c)
}

fn is_quote(c: char) -> bool {
    c == '\'' || c == '"'
}

pub fn split_args(args_str: &str) -> Result<Vec<String>> {
    let mut args = vec![];
    let mut remaining_str = args_str;
    while !remaining_str.is_empty() {
        let result = parse_quoted_arg(remaining_str);
        let (new_remaining, arg) = result?;
        remaining_str = new_remaining;
        args.push(arg);
    }
    Ok(args)
}

pub async fn initialize_command(
    ctx: &BotContext,
    data: InsertCommandAttributes<'static>,
    required_permission_names: Vec<impl AsRef<str> + Send + 'static>,
    aliases: Vec<impl AsRef<str> + Send + Sync + 'static>,
) -> Result<()> {
    let required_permissions: Vec<i32> = ctx
        .permissions
        .load()
        .get_permissions(required_permission_names.iter().map(|s| s.as_ref()))?
        .iter()
        .map(|permission| permission.id)
        .collect();

    persistence::commands::util::initialize_command(
        &ctx.db_context,
        data,
        required_permissions,
        aliases,
    )
    .await
    .map_err(Into::into)
}

pub fn parse_quoted_arg(input: &str) -> Result<(&str, String)> {
    let mut escape = false;
    let mut quote_state: Option<char> = None;
    let mut word_begin: Option<usize> = None;
    let mut word_end: Option<usize> = None;
    let mut remaining_begin: Option<usize> = None;

    let mut unescaped_output = String::new();

    for (i, current_char) in input.chars().enumerate() {
        let is_escape_char = current_char == '\\' && !escape;
        if word_begin.is_none() {
            if is_quote(current_char) && !escape {
                quote_state = Some(current_char);
            } else if char::is_whitespace(current_char) && !escape {
                continue;
            } else {
                word_begin = Some(i);
                if !is_escape_char && !disallowed_input_chars(current_char) {
                    unescaped_output.push(current_char);
                }
            }
        } else if is_quote(current_char) && !escape {
            if quote_state == Some(current_char) {
                quote_state = None;
                word_end = Some(i);
                remaining_begin = Some(min(i + 1, input.len()))
            } else {
                return Err(CommandError::QuoteMismatch.into());
            }
        } else if !escape && char::is_whitespace(current_char) && quote_state.is_none() {
            word_end.get_or_insert(i);
            break;
        } else if !is_escape_char && !disallowed_input_chars(current_char) {
            unescaped_output.push(current_char);
        }
        if escape {
            escape = false;
        }
        if is_escape_char {
            escape = true;
        }
    }
    match (word_begin, word_end) {
        (Some(_begin), Some(end)) => {
            Ok((&input[remaining_begin.unwrap_or(end)..], unescaped_output))
        }
        (Some(_begin), None) => {
            if quote_state.is_some() {
                Err(CommandError::QuoteMismatch.into())
            } else {
                Ok(("", unescaped_output))
            }
        }
        (None, _) => Ok((input, unescaped_output)),
    }
}

#[cfg(test)]
mod test {
    use crate::util::{parse_quoted_arg, split_args};

    #[test]
    fn test_quote_parser() {
        assert_eq!(parse_quoted_arg("test").unwrap(), ("", "test".to_string()));
        assert_eq!(
            parse_quoted_arg(r#""test""#).unwrap(),
            ("", "test".to_string())
        );
        assert_eq!(
            parse_quoted_arg(r#""test1" test2"#).unwrap(),
            (" test2", "test1".to_string())
        );
        assert_eq!(
            parse_quoted_arg(r#"'test1' test2"#).unwrap(),
            (" test2", "test1".to_string())
        );
        assert_eq!(
            parse_quoted_arg(r#"test1 test2"#).unwrap(),
            (" test2", "test1".to_string())
        );
        assert_eq!(
            parse_quoted_arg(r#""test1 2 3" test2"#).unwrap(),
            (" test2", "test1 2 3".to_string())
        );
        assert_eq!(
            parse_quoted_arg(r#""test1\" 2 3" test2"#).unwrap(),
            (" test2", "test1\" 2 3".to_string())
        );
        assert_eq!(
            parse_quoted_arg(r#"\"te\'st1\" test2"#).unwrap(),
            (" test2", r#""te'st1""#.to_string())
        );
    }

    #[test]
    fn test_split_quotes() {
        assert_eq!(
            split_args(r#"arg1 "arg 2" arg3 --opt arg\ 4"#).unwrap(),
            vec!["arg1", "arg 2", "arg3", "--opt", "arg 4"]
        )
    }
}
