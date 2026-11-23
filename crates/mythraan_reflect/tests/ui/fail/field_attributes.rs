use mythraan_reflect::Reflect;

#[derive(Reflect)]
struct Unrecognized {
    #[reflect(unknown)]
    value: f32,
}

#[derive(Reflect)]
struct Duplicate {
    #[reflect(hidden, hidden)]
    value: f32,
}

#[derive(Reflect)]
struct DuplicateRange {
    #[reflect(range = 0..1)]
    #[reflect(range = 0..=2)]
    value: f32,
}

#[derive(Reflect)]
struct Backwards {
    #[reflect(range = 2.0..=1.0)]
    value: f32,
}

#[derive(Reflect)]
struct NonFinite {
    #[reflect(range = 0..1e999)]
    value: f32,
}

#[derive(Reflect)]
#[reflect(hidden)]
struct Misplaced {
    value: f32,
}

fn main() {}
