use std::any::{TypeId, type_name};

use crate::Reflect;

/// A finite numeric range hint for a reflected field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldRange {
    min_bits: u64,
    max_bits: u64,
    inclusive: bool,
}

impl FieldRange {
    /// Constructs a range with an included lower bound.
    ///
    /// Returns `None` for non-finite bounds or an empty range.
    #[must_use]
    pub const fn new(min: f64, max: f64, inclusive: bool) -> Option<Self> {
        if !min.is_finite() || !max.is_finite() || min > max || (!inclusive && min == max) {
            return None;
        }
        Some(Self {
            min_bits: min.to_bits(),
            max_bits: max.to_bits(),
            inclusive,
        })
    }

    /// Returns the included lower bound.
    #[must_use]
    pub const fn min(&self) -> f64 {
        f64::from_bits(self.min_bits)
    }

    /// Returns the upper bound.
    #[must_use]
    pub const fn max(&self) -> f64 {
        f64::from_bits(self.max_bits)
    }

    /// Returns whether the upper bound is included.
    #[must_use]
    pub const fn is_inclusive(&self) -> bool {
        self.inclusive
    }
}

/// The name and Rust type of a reflected field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldInfo {
    name: &'static str,
    type_id: TypeId,
    range: Option<FieldRange>,
    hidden: bool,
}

impl FieldInfo {
    /// Describes a field with the given name and Rust type.
    #[must_use]
    pub const fn new<T: 'static>(name: &'static str) -> Self {
        Self {
            name,
            type_id: TypeId::of::<T>(),
            range: None,
            hidden: false,
        }
    }

    /// Returns the field's name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the Rust type identifier of the field's value.
    #[must_use]
    pub const fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Sets the field's optional numeric range hint.
    #[must_use]
    pub const fn with_range(mut self, range: Option<FieldRange>) -> Self {
        self.range = range;
        self
    }

    /// Marks the field as hidden in the inspector.
    #[must_use]
    pub const fn with_hidden(mut self) -> Self {
        self.hidden = true;
        self
    }

    /// Returns the field's numeric range hint, if any.
    #[must_use]
    pub const fn range(&self) -> Option<FieldRange> {
        self.range
    }

    /// Returns whether the field is marked as hidden in the inspector.
    #[must_use]
    pub const fn is_hidden(&self) -> bool {
        self.hidden
    }
}

/// Runtime metadata for one reflected Rust type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypeInfo {
    name: &'static str,
    type_id: TypeId,
    fields: &'static [FieldInfo],
    variants: &'static [&'static str],
}

impl TypeInfo {
    /// Gets the type's identity from Rust and its fields from its reflection implementation.
    #[must_use]
    pub fn of<T: Reflect>() -> Self {
        Self {
            name: type_name::<T>(),
            type_id: TypeId::of::<T>(),
            fields: T::fields(),
            variants: T::variants(),
        }
    }

    /// Returns the Rust type name used for lookup in this build.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the type's Rust identifier.
    #[must_use]
    pub const fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Returns field metadata in declaration order.
    #[must_use]
    pub const fn fields(&self) -> &'static [FieldInfo] {
        self.fields
    }

    /// Returns unit variant names in declaration order.
    #[must_use]
    pub const fn variants(&self) -> &'static [&'static str] {
        self.variants
    }
}
