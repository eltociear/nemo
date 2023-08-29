use std::collections::BTreeMap;

use super::{Identifier, Term};

/// A key in a [Map].
#[derive(Debug, Eq, PartialEq, Clone, Hash, PartialOrd, Ord)]
pub enum Key {
    /// A string key.
    String(String),
    /// An identifier key.
    Identifier(Identifier),
}

impl Key {
    /// Construct a new [Key] from a [String].
    pub fn string(s: String) -> Self {
        Self::String(s)
    }

    /// Construct a new [Key] from an [Identifier].
    pub fn identifier(i: Identifier) -> Self {
        Self::Identifier(i)
    }
}

impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Key::String(s) => write!(f, "{s}"),
            Key::Identifier(i) => write!(f, "{i}"),
        }
    }
}

/// A Map: a [Term] assigning values (which can be arbitrary
/// [Terms][Term]) to [Keys][Key].
#[derive(Debug, Eq, PartialEq, Clone, PartialOrd, Ord, Default)]
pub struct Map {
    pub(crate) pairs: BTreeMap<Key, Term>,
}

impl Map {
    /// Construct an empty [Map].
    pub fn new() -> Self {
        Default::default()
    }

    /// An iterator over the pairs in the map
    pub fn iter(&self) -> impl Iterator<Item = (&Key, &Term)> {
        self.pairs.iter()
    }
}

impl std::fmt::Display for Map {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ ")?;

        for (key, value) in &self.pairs {
            write!(f, "{key}: {value}")?;
        }

        write!(f, " }}")
    }
}

impl FromIterator<(Key, Term)> for Map {
    fn from_iter<T: IntoIterator<Item = (Key, Term)>>(iter: T) -> Self {
        Self {
            pairs: iter.into_iter().collect(),
        }
    }
}

impl From<Vec<(Key, Term)>> for Map {
    fn from(pairs: Vec<(Key, Term)>) -> Self {
        pairs.into_iter().collect()
    }
}