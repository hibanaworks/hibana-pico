//! Generic site substrate facts.
//!
//! Core site vocabulary is intentionally small. Users and examples define their
//! own execution substrate and carrier implementations with the same public
//! `LogicalImage` contract used by built-in examples.

use core::marker::PhantomData;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Local<Image>(PhantomData<Image>);

impl<Image> Local<Image> {
    pub const fn new() -> Self {
        Self(PhantomData)
    }
}
