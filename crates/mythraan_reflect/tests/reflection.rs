//! Public reflection API tests using manually reflected types.

use std::any::{TypeId, type_name};

use mythraan_reflect::prelude::{FieldInfo, Reflect, TypeInfo};

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
