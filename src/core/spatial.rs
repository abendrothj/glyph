//! Spatial index systems for viewport culling.

use bevy::prelude::*;

use crate::core::components::{CanvasNode, GridCell};
use crate::core::resources::SpatialIndex;

/// Keeps SpatialIndex in sync with CanvasNode transforms. Runs in PostUpdate.
pub fn update_spatial_index_system(
    mut spatial_index: ResMut<SpatialIndex>,
    mut commands: Commands,
    moved_nodes: Query<
        (Entity, &Transform, Option<&GridCell>),
        (With<CanvasNode>, Or<(Added<CanvasNode>, Changed<Transform>)>),
    >,
) {
    for (entity, transform, grid_cell) in &moved_nodes {
        let new_cell = SpatialIndex::world_to_cell(transform.translation);

        if let Some(&GridCell(old_x, old_y)) = grid_cell {
            let old_cell = (old_x, old_y);
            if old_cell != new_cell {
                spatial_index.remove(entity);
            }
        }

        spatial_index.insert(entity, new_cell);
        commands.entity(entity).insert(GridCell(new_cell.0, new_cell.1));
    }
}

/// Removes despawned or component-stripped entities from the spatial index.
pub fn spatial_index_cleanup_system(
    mut spatial_index: ResMut<SpatialIndex>,
    mut removed: RemovedComponents<CanvasNode>,
) {
    for entity in removed.read() {
        spatial_index.remove(entity);
    }
}
