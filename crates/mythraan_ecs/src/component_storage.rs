use std::any::Any;

use crate::SparseSet;

pub(crate) trait ComponentStorage: Any + Send + Sync {}

impl<T: Send + Sync + 'static> ComponentStorage for SparseSet<T> {}
