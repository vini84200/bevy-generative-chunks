use std::collections::HashMap;
use crate::generative_chunks::bounds::{ChunkIdx, Point};
use crate::generative_chunks::layer::Dependency;
use crate::generative_chunks::layer_id::LayerId;
use crate::generative_chunks::usage::UsageStrategy;

#[derive(Debug)]
pub struct LayerClient {
    active: bool,
    center: Point,
    dependencies: Vec<Dependency>,
    strategy: UsageStrategy,
}

impl IntoLayerClient for LayerClient {
    fn into_layer_client(self) -> LayerClient {
        self
    }
}

impl LayerClient {
    pub fn new(
        center: Point,
        dependencies: Vec<Dependency>,
        strength: UsageStrategy,
    ) -> Self {
        LayerClient {
            active: true,
            center,
            dependencies,
            strategy: strength,
        }
    }

    pub fn activate(&mut self) {
        self.active = true;
    }
    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn get_center(&self) -> Point {
        self.center
    }

    pub fn get_dependencies(&self) -> &Vec<Dependency> {
        &self.dependencies
    }
    
    pub fn get_strategy(&self) -> UsageStrategy {
        self.strategy
    }
}

pub trait IntoLayerClient {
    fn into_layer_client(self) -> LayerClient;
}
