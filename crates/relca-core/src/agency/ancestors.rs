//! Transitive binding ancestry: bounded words, not a cloned tree per edge.
use super::*;

#[derive(Clone)]
pub(super) enum Ancestors {
    Bits(BTreeMap<usize, u64>),
    #[cfg(test)]
    Reference(BTreeSet<ir::BindingId>),
}
impl Ancestors {
    pub fn new(_n: usize, _reference: bool, _b: &mut Budget) -> Result<Self> {
        #[cfg(test)]
        if _reference {
            return Ok(Self::Reference(BTreeSet::new()));
        }
        // Empty/shallow graphs must not reserve a dense n*n/64 matrix.
        // Allocate only words that actually contain an ancestor.
        Ok(Self::Bits(BTreeMap::new()))
    }
    pub fn inherit(all: &mut [Self], to: usize, from: ir::BindingId, b: &mut Budget) -> Result<()> {
        if to == from.0 {
            return Err(invalid());
        }
        let (child, parent) = if to < from.0 {
            let (left, right) = all.split_at_mut(from.0);
            (&mut left[to], &right[0])
        } else {
            let (left, right) = all.split_at_mut(to);
            (&mut right[0], &left[from.0])
        };
        match (child, parent) {
            (Self::Bits(child), Self::Bits(parent)) => {
                b.steps(parent.len())?;
                for (index, word) in parent {
                    if let Some(old) = child.get_mut(index) {
                        *old |= *word;
                    } else {
                        b.cells(1)?;
                        child.insert(*index, *word);
                    }
                }
                b.work()?;
                let index = from.0 / 64;
                let bit = 1u64 << (from.0 % 64);
                if let Some(word) = child.get_mut(&index) {
                    *word |= bit;
                } else {
                    b.cells(1)?;
                    child.insert(index, bit);
                }
                Ok(())
            }
            #[cfg(test)]
            (Self::Reference(child), Self::Reference(parent)) => {
                b.copy(parent.len())?;
                for id in parent.clone() {
                    b.insert(child, id)?;
                }
                b.insert(child, from)
            }
            #[cfg(test)]
            _ => Err(invalid()),
        }
    }
    pub fn contains(&self, id: &ir::BindingId) -> bool {
        match self {
            Self::Bits(words) => words
                .get(&(id.0 / 64))
                .is_some_and(|w| w & (1u64 << (id.0 % 64)) != 0),
            #[cfg(test)]
            Self::Reference(set) => set.contains(id),
        }
    }
    pub fn iter(&self) -> Iter<'_> {
        match self {
            Self::Bits(words) => Iter::Bits {
                words: words.iter(),
                base: 0,
                remaining: 0,
            },
            #[cfg(test)]
            Self::Reference(set) => Iter::Reference(set.iter()),
        }
    }
}
pub(super) enum Iter<'a> {
    Bits {
        words: std::collections::btree_map::Iter<'a, usize, u64>,
        base: usize,
        remaining: u64,
    },
    #[cfg(test)]
    Reference(std::collections::btree_set::Iter<'a, ir::BindingId>),
}
impl Iterator for Iter<'_> {
    type Item = ir::BindingId;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Bits {
                words,
                base,
                remaining,
            } => loop {
                if *remaining != 0 {
                    let bit = remaining.trailing_zeros() as usize;
                    *remaining &= *remaining - 1;
                    return Some(ir::BindingId(*base + bit));
                }
                let (index, word) = words.next()?;
                *remaining = *word;
                *base = *index * 64;
            },
            #[cfg(test)]
            Self::Reference(iter) => iter.next().copied(),
        }
    }
}
