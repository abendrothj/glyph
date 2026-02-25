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

/// The edge entity currently selected for label editing. Cleared when selecting a node or empty space.
#[derive(Resource, Default)]
pub struct SelectedEdge(pub Option<Entity>);

/// Transient status / error message displayed in the bottom bar.
/// `timer` counts down in seconds; the message is visible while `timer > 0`.
#[derive(Resource, Default)]
pub struct StatusMessage {
    pub text: String,
    pub timer: f32,
}

impl StatusMessage {
    pub fn set(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.timer = 4.0;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(i: u32) -> Entity {
        Entity::from_bits(i as u64)
    }

    #[test]
    fn world_to_cell() {
        assert_eq!(SpatialIndex::world_to_cell(Vec3::ZERO), (0, 0));
        assert_eq!(SpatialIndex::world_to_cell(Vec3::new(CELL_SIZE, 0.0, 0.0)), (1, 0));
        assert_eq!(SpatialIndex::world_to_cell(Vec3::new(0.0, CELL_SIZE, 0.0)), (0, 1));
        assert_eq!(SpatialIndex::world_to_cell(Vec3::new(1500.0, -500.0, 0.0)), (1, -1));
    }

    #[test]
    fn insert_remove_entities_in_bounds() {
        let mut idx = SpatialIndex::default();
        idx.insert(entity(1), (0, 0));
        idx.insert(entity(2), (0, 0));
        idx.insert(entity(3), (1, 0));

        // Bounds (0, CELL_SIZE-1) only include cell (0,0); cell (1,0) starts at CELL_SIZE
        let in_bounds = idx.entities_in_bounds(0.0, CELL_SIZE - 1.0, 0.0, CELL_SIZE - 1.0);
        assert_eq!(in_bounds.len(), 2);
        assert!(in_bounds.contains(&entity(1)));
        assert!(in_bounds.contains(&entity(2)));

        idx.remove(entity(1));
        let in_bounds = idx.entities_in_bounds(0.0, CELL_SIZE - 1.0, 0.0, CELL_SIZE - 1.0);
        assert_eq!(in_bounds.len(), 1);
        assert_eq!(in_bounds[0], entity(2));
    }

    #[test]
    fn clear() {
        let mut idx = SpatialIndex::default();
        idx.insert(entity(1), (0, 0));
        idx.clear();
        assert!(idx.entities_in_bounds(-1000.0, 1000.0, -1000.0, 1000.0).is_empty());
    }

    #[test]
    fn two_entities_same_cell_remove_one() {
        let mut idx = SpatialIndex::default();
        idx.insert(entity(1), (0, 0));
        idx.insert(entity(2), (0, 0));
        idx.remove(entity(1));
        let results = idx.entities_in_bounds(0.0, CELL_SIZE - 1.0, 0.0, CELL_SIZE - 1.0);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], entity(2));
    }

    #[test]
    fn entity_migration_between_cells() {
        let mut idx = SpatialIndex::default();
        idx.insert(entity(1), (0, 0));
        // Re-insert at a different cell (migration)
        idx.insert(entity(1), (1, 0));
        // Old cell should be empty
        let old_cell = idx.entities_in_bounds(0.0, CELL_SIZE - 1.0, 0.0, CELL_SIZE - 1.0);
        assert!(old_cell.is_empty());
        // New cell should have the entity
        let new_cell = idx.entities_in_bounds(CELL_SIZE, 2.0 * CELL_SIZE - 1.0, 0.0, CELL_SIZE - 1.0);
        assert_eq!(new_cell.len(), 1);
        assert_eq!(new_cell[0], entity(1));
    }

    #[test]
    fn negative_cell_coordinates() {
        let mut idx = SpatialIndex::default();
        idx.insert(entity(1), (-1, -1));
        let results = idx.entities_in_bounds(-CELL_SIZE, -1.0, -CELL_SIZE, -1.0);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], entity(1));
    }

    #[test]
    fn zero_width_bounds() {
        let mut idx = SpatialIndex::default();
        idx.insert(entity(1), (0, 0));
        // Bounds where min == max (single point query)
        let results = idx.entities_in_bounds(0.0, 0.0, 0.0, 0.0);
        assert_eq!(results.len(), 1);
    }
}
