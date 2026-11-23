use std::any::TypeId;

use mythraan_reflect::prelude::{Reflect, TypeInfo};

#[derive(Reflect)]
struct Settings<T, const N: usize>
where
    T: Copy,
{
    #[reflect(hidden)]
    elements: [T; N],
    #[reflect(range = -1..=1)]
    strength: f32,
}

#[derive(Reflect)]
enum Mode {
    On,
    Off,
}

fn main() {
    let settings = Settings {
        elements: [0_u16; 4],
        strength: 0.5,
    };
    assert_eq!(settings.elements.len(), 4);
    assert_eq!(settings.strength, 0.5);
    let fields = TypeInfo::of::<Settings<u16, 4>>().fields();
    assert!(fields[0].is_hidden());
    assert_eq!(fields[0].type_id(), TypeId::of::<[u16; 4]>());
    assert!(fields[1].range().unwrap().is_inclusive());
    let _modes = [Mode::On, Mode::Off];
    assert_eq!(TypeInfo::of::<Mode>().variants(), ["On", "Off"]);
}
