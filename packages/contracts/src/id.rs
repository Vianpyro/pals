//! Identifier of a course or an exercise, safe to put in a server path (ADR-0023).

use std::{error::Error, fmt, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, de};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, TS)]
#[ts(export)]
pub struct Id(String);

impl Id {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Id {
    type Error = InvalidId;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if is_valid(&value) {
            Ok(Self(value))
        } else {
            Err(InvalidId(value))
        }
    }
}

// By hand rather than `#[serde(try_from = "String")]`, which ts-rs cannot read and warns about on every build
impl<'de> Deserialize<'de> for Id {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .try_into()
            .map_err(de::Error::custom)
    }
}

impl FromStr for Id {
    type Err = InvalidId;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.to_owned().try_into()
    }
}

fn is_valid(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'-'))
}

impl fmt::Display for InvalidId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid identifier {:?}: expected [a-z0-9-]+", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidId(String);

impl Error for InvalidId {}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn assert_all_invalid(values: &[&str]) {
        for value in values {
            assert!(value.parse::<Id>().is_err(), "{value:?} was accepted");
        }
    }

    #[test]
    fn keeps_a_valid_identifier() {
        assert_eq!(
            "log200-lab-1".parse::<Id>().unwrap().as_str(),
            "log200-lab-1"
        );
    }

    #[test]
    fn json_is_checked_like_a_path() {
        let id: Id = serde_json::from_str(r#""sum""#).unwrap();
        assert_eq!(id.as_str(), "sum");
        let error = serde_json::from_str::<Id>(r#""../sum""#).unwrap_err();
        assert!(error.to_string().contains("invalid identifier"), "{error}");
    }

    #[test]
    fn empty_is_invalid() {
        assert_all_invalid(&[""]);
    }

    #[test]
    fn cannot_escape_the_course() {
        assert_all_invalid(&[
            ".",
            "..",
            "./foo",
            "../foo",
            "../../etc/passwd",
            "../log200/x", // another course's file (V-0023)
            "foo/../bar",
        ]);
    }

    #[test]
    fn windows_paths_are_invalid() {
        assert_all_invalid(&[
            r"..\foo",
            r"..\..\Windows\System32",
            r"C:\Windows\System32",
            r"C:/Windows/System32",
            r"\\server\share",
            r"\\?\C:\Windows",
        ]);
    }

    #[test]
    fn special_path_characters_are_invalid() {
        assert_all_invalid(&["/", "\\", ":", "*", "?", "\""]);
    }

    #[test]
    fn whitespace_and_controls_are_invalid() {
        assert_all_invalid(&["\0", "\n", "\r", "\t", "foo\0", "foo\n", " foo"]);
    }

    #[test]
    fn encoded_separators_are_invalid() {
        assert_all_invalid(&["%2e%2e", "%2f", "%5c"]);
    }

    #[test]
    fn uppercase_and_non_ascii_are_invalid() {
        assert_all_invalid(&["Sum", "énoncé", "ѕum"]); // Cyrillic ѕ, looks like s
    }
}
