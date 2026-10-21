//! Strict worker decoding, rejecting duplicate object members at every depth.
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};
use std::fmt;

struct Unique(Value);
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        struct Any;
        impl<'de> Visitor<'de> for Any {
            type Value = Unique;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("JSON with unique object fields")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Unique, E> {
                Ok(Unique(Value::Bool(v)))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Unique, E> {
                Ok(Unique(Value::from(v)))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Unique, E> {
                Ok(Unique(Value::from(v)))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Unique, E> {
                serde_json::Number::from_f64(v)
                    .map(Value::Number)
                    .map(Unique)
                    .ok_or_else(|| E::custom("non-finite JSON number"))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Unique, E> {
                Ok(Unique(Value::from(v)))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Unique, E> {
                Ok(Unique(Value::String(v)))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
            fn visit_none<E: de::Error>(self) -> Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Unique, A::Error> {
                let mut values = Vec::new();
                while let Some(Unique(value)) = a.next_element()? {
                    values.push(value);
                }
                Ok(Unique(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Unique, A::Error> {
                let mut fields = Map::new();
                while let Some((key, Unique(value))) = a.next_entry::<String, Unique>()? {
                    if fields.insert(key, value).is_some() {
                        return Err(de::Error::custom("SAIL4214: duplicate JSON object field"));
                    }
                }
                Ok(Unique(Value::Object(fields)))
            }
        }
        decoder.deserialize_any(Any)
    }
}

fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, serde_json::Error> {
    let Unique(value) = serde_json::from_slice(bytes)?;
    serde_json::from_value(value)
}

/// Decodes one entire request, refusing duplicates, trailing bytes and unknown fields.
///
/// # Errors
/// Malformed or incompatible payload.
pub fn decode_request(bytes: &[u8]) -> Result<crate::Request, serde_json::Error> {
    decode(bytes)
}

/// Decodes one entire response, refusing duplicates, trailing bytes and unknown fields.
///
/// # Errors
/// Malformed or incompatible payload.
pub fn decode_response(bytes: &[u8]) -> Result<crate::Response, serde_json::Error> {
    decode(bytes)
}
