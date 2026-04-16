use bitcoin::Amount;

/// A type that carries a bitcoin [`Amount`].
pub trait Valued {
    fn value(&self) -> Amount;
}

/// Extension trait providing aggregate operations on slices of [`Valued`] items.
pub trait ValuedSlice {
    fn total_value(&self) -> Amount;
}

impl<T: Valued> ValuedSlice for [T] {
    fn total_value(&self) -> Amount {
        self.iter().map(|v| v.value()).sum()
    }
}
