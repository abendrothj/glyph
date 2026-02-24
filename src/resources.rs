//! Resources for the whiteboard.

use bevy::prelude::*;
use std::collections::HashMap;

/// World units per spatial grid cell. Tune for your typical node density.
pub const CELL_SIZE: f32 = 1000.0;

/// Maps single-character jump tags ('a', 'b', …) to their CanvasNode entity.
/// Populated on VimEasymotion entry; cleared on exit.
#[derive(Resource, Default)]
pub struct JumpMap {
    pub char_to_entity: HashMap<char, Entity>,
}

/// Spatial hash grid for O(1) viewport-culled lookups. Keys are (cell_x, cell_y).
#[derive(Resource, Default)]
pub struct SpatialIndex {
    cell_to_entities: HashMap<(i32, i32), Vec<Entity>>,
    entity_to_cell: HashMap<Entity, (i32, i32)>,
}

impl SpatialIndex {
    pub fn world_to_cell(translation: Vec3) -> (i32, i32) {
        (
            (translation.x / CELL_SIZE).floor() as i32,
            (translation.y / CELL_SIZE).floor() as i32,
        )
    }

    pub fn insert(&mut self, entity: Entity, cell: (i32, i32)) {
        self.remove(entity);
        self.entity_to_cell.insert(entity, cell);
        self.cell_to_entities
            .entry(cell)
            .or_default()
            .push(entity);
    }

    /// Clears the entire index. Used when loading a new canvas.
    pub fn clear(&mut self) {
        self.cell_to_entities.clear();
        self.entity_to_cell.clear();
    }

    pub fn remove(&mut self, entity: Entity) {
        if let Some(cell) = self.entity_to_cell.remove(&entity) {
            if let Some(entities) = self.cell_to_entities.get_mut(&cell) {
                entities.retain(|e| *e != entity);
                if entities.is_empty() {
                    self.cell_to_entities.remove(&cell);
                }
            }
        }
    }

    /// Returns entities in cells that intersect the given world-space AABB.
    pub fn entities_in_bounds(&self, min_x: f32, max_x: f32, min_y: f32, max_y: f32) -> Vec<Entity> {
        let min_cx = (min_x / CELL_SIZE).floor() as i32;
        let max_cx = (max_x / CELL_SIZE).floor() as i32;
        let min_cy = (min_y / CELL_SIZE).floor() as i32;
        let max_cy = (max_y / CELL_SIZE).floor() as i32;

        let mut out = Vec::new();
        for cx in min_cx..=max_cx {
            for cy in min_cy..=max_cy {
                if let Some(entities) = self.cell_to_entities.get(&(cx, cy)) {
                    out.extend(entities.iter().copied());
                }
            }
        }
        out
    }
}
