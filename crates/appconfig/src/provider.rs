use crate::Key;

/// Environment variable based provider.
pub mod env;

/// A config provider.
pub trait Provider {
    /// Returns the value at the given config path, if it exists.
    fn get(&self, key: &Key) -> anyhow::Result<Option<String>>;
}
