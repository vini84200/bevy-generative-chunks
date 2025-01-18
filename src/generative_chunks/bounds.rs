use bevy::prelude::Vec2;
use serde::Deserialize;

// Bounds are always in real coordinates
#[derive(Debug)]
pub struct Bounds {
    min: Vec2,
    max: Vec2,
}

impl Bounds {
    pub(crate) fn expand(&self, x: f32, y: f32) -> Bounds {
        Bounds {
            min: Vec2::new(self.min.x - x, self.min.y - y),
            max: Vec2::new(self.max.x + x, self.max.y + y),
        }
    }
}

impl Bounds {
    pub fn add_padding(&self, padding: Vec2) -> Bounds {
        Bounds::new(
            Vec2::new(self.min.x - padding.x, self.min.y - padding.y),
            Vec2::new(self.max.x + padding.x, self.max.y + padding.y),
        )
    }

    pub fn new(min: Point, max: Point) -> Bounds {
        Bounds { min, max }
    }

    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }

    pub fn intersects(&self, other: &Bounds) -> bool {
        self.contains(other.min)
            || self.contains(other.max)
            || other.contains(self.min)
            || other.contains(self.max)
    }

    pub fn get_center(&self) -> Point {
        Vec2::new((self.min.x + self.max.x) / 2.0, (self.min.y + self.max.y) / 2.0)
    }

    pub fn center_and_padding(center: (f32, f32), padding: (f32, f32)) -> (f32, f32) {
        (center.0 - padding.0, center.1 - padding.1)
    }

    pub fn from_point(point: Vec2) -> Bounds {
        Bounds::new(point, point)
    }

    pub fn add_point(self, point: (f32, f32)) -> Bounds {
        Bounds::new(
            Vec2::new(self.min.x.min(point.0), self.min.y.min(point.1)),
            Vec2::new(self.max.x.max(point.0), self.max.y.max(point.1)),
        )
    }

    pub fn chunks(&self, chunk_size: Point) -> impl Iterator<Item=ChunkIdx> {
        let min_chunk = (
            (self.min.x / chunk_size.x).floor() as i32,
            (self.min.y / chunk_size.y).floor() as i32,
        );
        let max_chunk = (
            (self.max.x / chunk_size.x).ceil() as i32,
            (self.max.y / chunk_size.y).ceil() as i32,
        );

        (min_chunk.0..=max_chunk.0)
            .flat_map(move |x| (min_chunk.1..=max_chunk.1).map(move |y| ChunkIdx { x, y }))
    }
}

#[derive(Debug, Deserialize, Hash, Eq, PartialEq, Clone, Copy)]
pub struct ChunkIdx {
    pub x: i32,
    pub y: i32,
}

impl ChunkIdx {
    // pub(crate) fn to_point(&self, chunk_width: f32, chunk_height: f32) -> Point {
    //     (self.x as f32 * chunk_width, self.y as f32 * chunk_height)
    // }
    pub(crate) fn to_point(&self, chunk_size: Point) -> Point {
        Vec2::new(self.x as f32 * chunk_size.x,
                  self.y as f32 * chunk_size.y)
    }

    pub fn center(&self, chunk_size: Point) -> Point {
        self.to_point(chunk_size) + chunk_size / 2.0
    }
}

impl ChunkIdx {
    pub(crate) fn from_point(pos: Point, chunk_width: f32, chunk_height: f32) -> ChunkIdx {
        ChunkIdx {
            x: (pos.x / chunk_width).floor() as i32,
            y: (pos.y / chunk_height).floor() as i32,
        }
    }
}

impl ChunkIdx {
    pub(crate) fn to_bounds(&self, width: f32, height: f32) -> Bounds {
        Bounds::new(
            Vec2::new(self.x as f32 * width, self.y as f32 * height),
            Vec2::new((self.x + 1) as f32 * width, (self.y + 1) as f32 * height),
        )
    }
}

pub(crate) type Point = Vec2;
