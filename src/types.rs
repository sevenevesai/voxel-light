/// A pending light level change at a world-space position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LightUpdate {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub level: u8,
}

impl LightUpdate {
    pub fn new(pos: [i32; 3], level: u8) -> Self {
        Self {
            x: pos[0],
            y: pos[1],
            z: pos[2],
            level,
        }
    }

    pub fn pos(&self) -> [i32; 3] {
        [self.x, self.y, self.z]
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LightNode {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub level: u8,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RemovalNode {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub old_level: u8,
}

pub(crate) const DIRECTIONS: [[i32; 3]; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];
