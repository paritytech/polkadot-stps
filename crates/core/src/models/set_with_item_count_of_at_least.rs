use crate::prelude::*;

#[derive(Debug, Clone, Deref, AsRef, PartialEq, Eq, Hash)]
pub struct SetWithItemCountOfAtLeast<const N: usize, T: std::hash::Hash + std::cmp::Eq>(Vec<T>);
impl<const N: usize, T: std::hash::Hash + std::cmp::Eq> SetWithItemCountOfAtLeast<N, T> {
    /// # Panics
    /// Panics if `items` is does not have length `N`.
    pub fn new(items: impl IntoIterator<Item = T>) -> Self {
        let set_as_vec = items.into_iter().collect::<Vec<T>>();
        let len = set_as_vec.len();
        if len < N {
            panic!("Cannot create SetWithItemCountOfAtLeast with items of length {}, expected at least {}", len, N);
        } else {
            Self(set_as_vec)
        }
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl IntoIterator for SetWithItemCountOfAtLeast<1, Recipient> {
    type Item = Recipient;
    type IntoIter = std::vec::IntoIter<Recipient>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
