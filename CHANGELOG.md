# Changelog

## Unreleased

- ECS entity generations start at 1 (previously 0). On x86-64,
  `Option<Entity>` occupies 8 bytes (previously 12).
- `SparseSet` debug output shows occupied slots as `(dense_index, generation)`
  and empty slots as `None`.
