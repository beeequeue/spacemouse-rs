pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector3 {
    const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub const ZERO: Self = Self::new(0., 0., 0.);
}
