use std::any::{Any, TypeId};
use std::collections::HashMap;

use mythraan_reflect::{ReflectFields, RegisterTypeError, TypeRegistry};

use crate::{Entity, ReflectComponentError, World};

type ReadComponent = fn(&World, Entity) -> Option<&dyn ReflectFields>;

struct Access {
    read: ReadComponent,
}

#[derive(Default)]
pub(super) struct ReflectedComponents {
    types: TypeRegistry,
    access: HashMap<TypeId, Access>,
}

impl World {
    /// Registers a component type for metadata and named field access in this world.
    ///
    /// # Errors
    ///
    /// A type name already used by a different type is rejected.
    pub fn register_type<T: ReflectFields + Send + Sync>(
        &mut self,
    ) -> Result<(), RegisterTypeError> {
        self.reflected.types.register::<T>()?;
        self.reflected
            .access
            .entry(TypeId::of::<T>())
            .or_insert(Access {
                read: |world, entity| {
                    world
                        .get::<T>(entity)
                        .map(|component| component as &dyn ReflectFields)
                },
            });
        Ok(())
    }

    /// Returns the metadata registry for this world.
    #[must_use]
    pub fn type_registry(&self) -> &TypeRegistry {
        &self.reflected.types
    }

    /// Borrows a named field from a registered component on an entity.
    ///
    /// Downcast the returned value with [`Any::downcast_ref`].
    ///
    /// # Errors
    ///
    /// Returns an error for an unregistered type, a missing component,
    /// or a field the component does not expose.
    pub fn reflected_field(
        &self,
        entity: Entity,
        component: TypeId,
        field: &str,
    ) -> Result<&dyn Any, ReflectComponentError> {
        let access = self
            .reflected
            .access
            .get(&component)
            .ok_or(ReflectComponentError::UnregisteredType(component))?;
        let component =
            (access.read)(self, entity).ok_or(ReflectComponentError::MissingComponent(entity))?;
        component
            .field(field)
            .ok_or_else(|| ReflectComponentError::UnknownField(field.to_owned()))
    }
}
