//! Public reflection API tests using manually reflected types.

use std::any::{TypeId, type_name};

use mythraan_reflect::RegisterTypeError;
use mythraan_reflect::prelude::{FieldInfo, Reflect, TypeInfo, TypeRegistry};

#[derive(Default)]
struct LightSettings {
    intensity: f32,
    enabled: bool,
}

impl Reflect for LightSettings {
    fn fields() -> &'static [FieldInfo] {
        const FIELDS: &[FieldInfo] = &[
            FieldInfo::new::<f32>("intensity"),
            FieldInfo::new::<bool>("enabled"),
        ];
        FIELDS
    }
}

struct Marker;

impl Reflect for Marker {
    fn fields() -> &'static [FieldInfo] {
        &[]
    }
}

#[test]
fn describes_named_fields_in_declaration_order() {
    fn value_type<T: 'static>(_: &T) -> TypeId {
        TypeId::of::<T>()
    }

    let info = TypeInfo::of::<LightSettings>();
    let settings = LightSettings::default();
    assert_eq!(info.name(), type_name::<LightSettings>());
    assert_eq!(info.type_id(), TypeId::of::<LightSettings>());
    let fields: Vec<_> = info
        .fields()
        .iter()
        .map(|field| (field.name(), field.type_id()))
        .collect();
    assert_eq!(
        fields,
        [
            ("intensity", value_type(&settings.intensity)),
            ("enabled", value_type(&settings.enabled)),
        ]
    );
}

#[test]
fn describes_types_without_fields() {
    let info = TypeInfo::of::<Marker>();
    assert_eq!(info.type_id(), TypeId::of::<Marker>());
    assert!(info.fields().is_empty());
}

#[test]
fn finds_registered_types_by_name_and_type_id() {
    let mut registry = TypeRegistry::new();
    registry.register::<LightSettings>().unwrap();
    registry.register::<Marker>().unwrap();

    for info in [TypeInfo::of::<LightSettings>(), TypeInfo::of::<Marker>()] {
        assert_eq!(registry.get_by_name(info.name()), Some(&info));
        assert_eq!(registry.get_by_id(info.type_id()), Some(&info));
    }
    assert!(registry.get_by_name("missing::Component").is_none());
    assert!(registry.get_by_id(TypeId::of::<u8>()).is_none());
}

#[test]
fn registering_the_same_type_again_succeeds() {
    let mut registry = TypeRegistry::new();
    registry.register::<Marker>().unwrap();
    registry.register::<Marker>().unwrap();

    assert_eq!(
        registry.get_by_name(type_name::<Marker>()),
        Some(&TypeInfo::of::<Marker>())
    );
}

#[test]
fn rejects_name_collisions_and_preserves_both_indexes() {
    let mut registry = TypeRegistry::new();

    let first = {
        struct Component;

        impl Reflect for Component {
            fn fields() -> &'static [FieldInfo] {
                &[]
            }
        }

        registry.register::<Component>().unwrap();
        TypeInfo::of::<Component>()
    };

    let (second, result) = {
        struct Component;

        impl Reflect for Component {
            fn fields() -> &'static [FieldInfo] {
                &[]
            }
        }

        (
            TypeInfo::of::<Component>(),
            registry.register::<Component>(),
        )
    };

    assert_ne!(first.type_id(), second.type_id());
    assert_eq!(first.name(), second.name());
    assert_eq!(
        result,
        Err(RegisterTypeError::NameCollision { name: first.name() })
    );
    assert_eq!(registry.get_by_name(first.name()), Some(&first));
    assert_eq!(registry.get_by_id(first.type_id()), Some(&first));
    assert!(registry.get_by_id(second.type_id()).is_none());
}
