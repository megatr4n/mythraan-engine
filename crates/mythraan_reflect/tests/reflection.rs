//! Public reflection API tests using manually reflected types.

use std::any::{TypeId, type_name};

use mythraan_reflect::RegisterTypeError;
use mythraan_reflect::prelude::{FieldInfo, FieldRange, Reflect, TypeInfo, TypeRegistry};

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

enum LightMode {
    On,
    Off,
}

impl Reflect for LightMode {
    fn fields() -> &'static [FieldInfo] {
        &[]
    }

    fn variants() -> &'static [&'static str] {
        &["On", "Off"]
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
    assert!(info.variants().is_empty());
}

#[test]
fn describes_unit_variants() {
    let modes = [LightMode::On, LightMode::Off];
    let info = TypeInfo::of::<LightMode>();
    assert_eq!(info.variants(), ["On", "Off"]);
    assert_eq!(info.variants().len(), modes.len());
}

#[test]
fn stores_visibility_and_valid_numeric_bounds() {
    let field = FieldInfo::new::<f32>("intensity");
    assert!(!field.is_hidden());
    assert_eq!(field.range(), None);

    let field = field
        .with_hidden()
        .with_range(FieldRange::new(-1.0, 1.0, false));
    let range = field.range().unwrap();
    assert!(field.is_hidden());
    assert_eq!(
        (range.min(), range.max(), range.is_inclusive()),
        (-1.0, 1.0, false)
    );
    assert!(FieldRange::new(1.0, 1.0, true).is_some());
    assert!(FieldRange::new(1.0, 1.0, false).is_none());
    assert!(FieldRange::new(2.0, 1.0, true).is_none());
    assert!(FieldRange::new(0.0, f64::INFINITY, true).is_none());
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
