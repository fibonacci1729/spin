use serde::{Deserialize, Serialize};

/// FixedVersion represents a version integer field with a const value.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(into = "usize", try_from = "usize")]
pub struct FixedVersion<const V: usize>;

impl<const V: usize> From<FixedVersion<V>> for usize {
    fn from(_: FixedVersion<V>) -> usize {
        V
    }
}

impl<const V: usize> TryFrom<usize> for FixedVersion<V> {
    type Error = String;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        if value != V {
            return Err(format!("invalid version {} != {}", value, V));
        }
        Ok(Self)
    }
}

/// FixedStringVersion represents a version string field with a const value.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct FixedStringVersion<const V: usize>;

impl<const V: usize> From<FixedStringVersion<V>> for String {
    fn from(_: FixedStringVersion<V>) -> String {
        V.to_string()
    }
}

impl<const V: usize> TryFrom<String> for FixedStringVersion<V> {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.parse() != Ok(V) {
            return Err(format!("invalid version {value:?} != \"{V}\""));
        }
        Ok(Self)
    }
}
