use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::mem;

use mythraan_reflect::{ReflectFields, RegisterTypeError, TypeRegistry};

use crate::change_detection::ComponentTicks;
use crate::{Entity, ReflectComponentError, World};

type ReadComponent = fn(&World, Entity) -> Option<&dyn ReflectFields>;
type WriteComponent =
    fn(&mut World, Entity) -> Option<(&mut dyn ReflectFields, Option<&ComponentTicks>)>;

struct Access {
    read: ReadComponent,
    write: WriteComponent,
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
                write: |world, entity| {
                    let (component, ticks) =
                        world.storage_mut::<T>(entity)?.get_mut_with_ticks(entity)?;
                    Some((component, ticks))
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
    /// Downcast the returned value with `downcast_ref`.
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

    /// Replaces a named field and marks the component changed on success.
    ///
    /// ```
    /// use std::any::type_name;
    /// use mythraan_ecs::{World, reflection::Reflect};
    ///
    /// #[derive(Reflect)]
    /// struct Health {
    ///     points: u32,
    /// }
    ///
    /// let mut world = World::new();
    /// world.register_type::<Health>()?;
    /// let entity = world.spawn()?;
    /// world.insert(entity, Health { points: 100 })?;
    /// let info = world.type_registry().get_by_name(type_name::<Health>())
    ///     .ok_or("missing Health metadata")?;
    /// let id = info.type_id();
    /// world.set_reflected_field(entity, id, "points", 80_u32)?;
    /// assert_eq!(world.reflected_field(entity, id, "points")?.downcast_ref(), Some(&80_u32));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the type, component or field is unavailable,
    /// or if the value has a different Rust type than the field.
    pub fn set_reflected_field<V: Any>(
        &mut self,
        entity: Entity,
        component: TypeId,
        name: &str,
        value: V,
    ) -> Result<(), ReflectComponentError> {
        let write = self
            .reflected
            .access
            .get(&component)
            .ok_or(ReflectComponentError::UnregisteredType(component))?
            .write;
        let tick = self.change_tick();
        let (reflected, ticks) =
            write(self, entity).ok_or(ReflectComponentError::MissingComponent(entity))?;
        let field = reflected
            .field_mut(name)
            .ok_or_else(|| ReflectComponentError::UnknownField(name.to_owned()))?;
        let Some(destination) = field.downcast_mut::<V>() else {
            return Err(ReflectComponentError::TypeMismatch {
                expected: Any::type_id(&*field),
                actual: TypeId::of::<V>(),
            });
        };
        let previous = mem::replace(destination, value);
        if let Some(ticks) = ticks {
            ticks.mark_changed(tick);
        }
        self.transforms.invalidate(component);
        drop(previous);
        Ok(())
    }
}
