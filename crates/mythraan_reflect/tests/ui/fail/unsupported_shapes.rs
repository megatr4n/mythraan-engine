use mythraan_reflect::Reflect;

#[derive(Reflect)]
struct Tuple(u32);

#[derive(Reflect)]
enum Data {
    Number(u32),
}

#[derive(Reflect)]
union Bits {
    integer: u32,
    float: f32,
}

fn main() {}
