//! Strict proof-event cursor arguments.
use sail_api::proof::Cursor;

pub(super) fn events(args: &[String]) -> Result<(String, Cursor, bool, bool), String> {
    let id = args
        .first()
        .filter(|id| !id.starts_with('-'))
        .ok_or("events requires JOB")?
        .clone();
    let (mut cursor, mut follow, mut jsonl) = (Cursor::default(), false, true);
    let mut iter = args[1..].iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--stream" => cursor
                .stream_id
                .clone_from(iter.next().ok_or("--stream requires ID")?),
            "--after" => {
                cursor.after = iter
                    .next()
                    .ok_or("--after requires N")?
                    .parse()
                    .map_err(|_| "invalid event cursor")?;
            }
            "--follow" => follow = true,
            "--output" => {
                jsonl = match iter.next().map(String::as_str) {
                    Some("jsonl") => true,
                    Some("json") => false,
                    _ => return Err("events output must be jsonl or json".into()),
                }
            }
            _ => return Err(format!("unknown event option {arg}")),
        }
    }
    if follow && !jsonl {
        return Err("follow requires --output jsonl".into());
    }
    Ok((id, cursor, follow, jsonl))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).into()).collect()
    }
    #[test]
    fn cursors_are_bounded_unsigned_and_follow_is_explicit_jsonl() {
        let (_, cursor, follow, jsonl) = events(&args(&[
            "job", "--stream", "job", "--after", "9", "--follow", "--output", "jsonl",
        ]))
        .unwrap_or_else(|error| unreachable!("{error}"));
        assert_eq!(cursor.after, 9);
        assert!(follow && jsonl);
        assert!(events(&args(&["job", "--after", "-1"])).is_err());
        assert!(events(&args(&["job", "--follow", "--output", "json"])).is_err());
    }
}
