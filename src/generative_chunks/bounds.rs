use serde::Deserialize;

// Bounds are always in real coordinates
#[derive(Debug)]
pub struct Bounds<T = f32> {
    min: (T, T),
    max: (T, T),
}

impl Bounds <f32>{

    pub fn add_padding(&self, padding: (f32, f32)) -> Bounds {
        Bounds::new(
            (self.min.0 - padding.0, self.min.1 - padding.1),
            (self.max.0 + padding.0, self.max.1 + padding.1),
        )
    }

    pub fn new(min: Point, max: Point) -> Bounds {
        Bounds { min, max }
    }

    pub fn contains(&self, point: Point) -> bool {
        point.0 >= self.min.0
            && point.0 <= self.max.0
            && point.1 >= self.min.1
            && point.1 <= self.max.1
    }

    pub fn intersects(&self, other: &Bounds) -> bool {
        self.contains(other.min)
            || self.contains(other.max)
            || other.contains(self.min)
            || other.contains(self.max)
    }

    pub fn get_center(&self) -> Point {
        ((self.min.0 + self.max.0) / 2.0, (self.min.1 + self.max.1) / 2.0)
    }

    pub fn center_and_padding(center: (f32, f32), padding: (f32, f32)) -> (f32, f32) {
        (center.0 - padding.0, center.1 - padding.1)
    }

    pub fn from_point(point: (f32, f32)) -> Bounds {
        Bounds::new(point, point)
    }

    pub fn add_point(self, point: (f32, f32)) -> Bounds {
        Bounds::new(
            (self.min.0.min(point.0), self.min.1.min(point.1)),
            (self.max.0.max(point.0), self.max.1.max(point.1)),
        )
    }

    pub fn chunks(&self, chunk_size: (f32, f32)) -> impl Iterator<Item = ChunkIdx> {
        let min_chunk = (
            (self.min.0 / chunk_size.0).floor() as u32,
            (self.min.1 / chunk_size.1).floor() as u32,
        );
        let max_chunk = (
            (self.max.0 / chunk_size.0).ceil() as u32,
            (self.max.1 / chunk_size.1).ceil() as u32,
        );

        (min_chunk.0..max_chunk.0)
            .flat_map(move |x| (min_chunk.1..max_chunk.1).map(move |y| ChunkIdx { x, y }))
    }
}

#[derive(Debug, Deserialize, Hash, Eq, PartialEq, Clone, Copy)]
pub struct ChunkIdx {
    pub x: u32,
    pub y: u32,
}

impl ChunkIdx {
    pub(crate) fn from_point(p0: Point, p1: f32, p2: f32) -> _ {
        ChunkIdx {
            x: (p0.0 / p1).floor() as u32,
            y: (p0.1 / p2).floor() as u32,
        }
    }
}

impl ChunkIdx {
    pub(crate) fn  to_bounds(&self, width: f32, height: f32) -> Bounds {
        Bounds::new(
            (self.x as f32 * width, self.y as f32 * height),
            ((self.x + 1) as f32 * width, (self.y + 1) as f32 * height),
        )
    }
}

pub(crate) type Point = (f32, f32);