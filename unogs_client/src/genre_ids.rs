use std::fmt;
use std::fmt::Debug;

use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, PartialEq)]
pub struct Genre {
    /// Title of the genre
    name: String,
    /// Netflix genre IDs belonging to that title
    ids: Vec<usize>,
}

struct GenreVisitor;
impl<'de> Visitor<'de> for GenreVisitor {
    type Value = Genre;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "map containing a single item with an array value"
        )
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, <A as MapAccess<'de>>::Error>
    where
        A: MapAccess<'de>,
    {
        return if let Some((name, ids)) = map.next_entry::<String, Vec<usize>>()? {
            Ok(Genre { name, ids })
        } else {
            Err(serde::de::Error::custom(
                "Expected map containing a single item with an array value",
            ))
        };
    }
}

impl<'de> Deserialize<'de> for Genre {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(GenreVisitor)
    }
}

#[cfg(test)]
mod tests {
    use crate::List;

    use super::*;

    #[test]
    fn test_genre_deserialize() {
        let json = r#"
        {
          "COUNT": "1",
          "ITEMS": [
            {
              "All Action": [
                111,
                222
              ]
            }
          ]
        }
        "#;
        let result: List<Genre> = serde_json::from_str(json).unwrap();

        assert_eq!(
            result,
            List {
                count: 1,
                items: vec![Genre {
                    name: "All Action".to_string(),
                    ids: vec![111, 222]
                }]
            }
        )
    }
}
