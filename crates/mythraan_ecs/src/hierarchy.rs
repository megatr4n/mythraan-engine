use crate::{Entity, HierarchyError, SparseSet};

/// The direct parent of an entity, borrowed through [`crate::World::parent`].
#[derive(Debug, PartialEq, Eq)]
pub struct Parent(Entity);

impl Parent {
    /// Returns the parent's entity identifier.
    #[must_use]
    pub const fn entity(&self) -> Entity {
        self.0
    }
}

/// The direct children of an entity, borrowed through [`crate::World::children`].
#[derive(Debug)]
pub struct Children(Vec<Entity>);

impl Children {
    /// Returns the child identifiers. Reparenting or removal may change their order.
    #[must_use]
    pub fn as_slice(&self) -> &[Entity] {
        &self.0
    }
}

#[derive(Default)]
pub(crate) struct Hierarchy {
    parents: SparseSet<Parent>,
    children: SparseSet<Children>,
}

impl Hierarchy {
    pub(crate) fn parent(&self, child: Entity) -> Option<&Parent> {
        self.parents.get(child)
    }

    pub(crate) fn children(&self, parent: Entity) -> Option<&Children> {
        self.children.get(parent)
    }

    pub(crate) fn roots(&self) -> impl Iterator<Item = Entity> + '_ {
        self.children
            .entities()
            .iter()
            .copied()
            .filter(|&entity| self.parent(entity).is_none())
    }

    pub(crate) fn set_parent(
        &mut self,
        child: Entity,
        parent: Option<Entity>,
    ) -> Result<(), HierarchyError> {
        let old = self.parent(child).map(Parent::entity);
        if old == parent {
            return Ok(());
        }
        if let Some(parent) = parent {
            let mut ancestor = Some(parent);
            while let Some(entity) = ancestor {
                if entity == child {
                    return Err(HierarchyError::Cycle(child));
                }
                ancestor = self.parent(entity).map(Parent::entity);
            }
            if let Some(children) = self.children.get_mut(parent) {
                children.0.try_reserve(1)?;
                self.parents.insert(child, Parent(parent))?;
                children.0.push(child);
            } else {
                let mut children = Vec::new();
                children.try_reserve(1)?;
                children.push(child);
                self.children.insert(parent, Children(children))?;
                if let Err(error) = self.parents.insert(child, Parent(parent)) {
                    self.children.remove(parent);
                    return Err(error.into());
                }
            }
        } else {
            self.parents.remove(child);
        }
        if let Some(old) = old {
            self.unlink(old, child);
        }
        Ok(())
    }

    pub(crate) fn detach(&mut self, child: Entity) -> Option<Entity> {
        let parent = self.parents.remove(child)?.entity();
        self.unlink(parent, child);
        Some(parent)
    }

    fn unlink(&mut self, parent: Entity, child: Entity) {
        if let Some(children) = self.children.get_mut(parent) {
            if children.0.last() == Some(&child) {
                children.0.pop();
            } else if let Some(index) = children.0.iter().position(|&entity| entity == child) {
                children.0.swap_remove(index);
            }
            if children.0.is_empty() {
                self.children.remove(parent);
            }
        }
    }
}
