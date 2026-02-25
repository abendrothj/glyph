//! Visual module clustering: gives each crawled function node a subtle
//! colored backdrop that identifies which source file it belongs to.
//!
//! Instead of one big bounding-box rectangle per file (which becomes enormous
//! when nodes from the same file span many hierarchy levels), each individual
//! node gets its own slightly-oversized colored sprite rendered behind it.
//! Nodes from the same file share the same hue, so file groupings are
//! immediately apparent without cluttering the canvas.
//!
//! The backdrop sprite is attached as a **child** of the CanvasNode entity so
//! it automatically moves with the node and is despawned when the node is.

use bevy::prelude::*;

use crate::core::components::{CanvasNode, SourceLocation};
use crate::core::helpers::NODE_SIZE;

/// Marker on the colored backdrop sprite child of a crawled node.
#[derive(Component)]
pub struct ClusterBlob;

/// Marker added to a CanvasNode once its halo child has been spawned,
/// preventing the system from adding a second halo on the next frame.
#[derive(Component)]
pub struct HasClusterBlob;

/// How much larger (in world units, per side) the halo is than the node itself.
const HALO_PAD: f32 = 10.0;

/// Palette of muted, distinct (r, g, b) triples for the per-file colors.
const PALETTE: &[(f32, f32, f32)] = &[
    (0.25, 0.50, 0.90), // blue
    (0.20, 0.72, 0.42), // green
    (0.85, 0.35, 0.25), // red-orange
    (0.75, 0.50, 0.12), // amber
    (0.60, 0.22, 0.80), // purple
    (0.15, 0.68, 0.72), // teal
    (0.88, 0.68, 0.18), // gold
    (0.35, 0.35, 0.78), // indigo
];

/// Stable per-file color derived from the absolute file path (Bevy sprite alpha).
fn halo_color(file: &str) -> Color {
    let (r, g, b) = palette_rgb(file);
    Color::srgba(r, g, b, 0.45)
}

/// The (r,g,b) palette entry for a file path — shared with the legend.
pub fn palette_rgb(file: &str) -> (f32, f32, f32) {
    let h = file.bytes().fold(0usize, |acc, b| acc.wrapping_mul(31).wrapping_add(b as usize));
    PALETTE[h % PALETTE.len()]
}

/// Runs once per new crawled node: attaches a colored halo child sprite.
/// Uses the `HasClusterBlob` marker to run only for nodes that don't yet
/// have a halo, so the query body executes O(nodes) total, not per frame.
pub fn cluster_blobs_system(
    mut commands: Commands,
    node_query: Query<(Entity, &SourceLocation), (With<CanvasNode>, Without<HasClusterBlob>)>,
) {
    for (entity, loc) in &node_query {
        let color = halo_color(&loc.file);
        let halo_size = NODE_SIZE + Vec2::splat(HALO_PAD * 2.0);
        commands.entity(entity)
            .insert(HasClusterBlob)
            .with_children(|parent| {
                parent.spawn((
                    Sprite {
                        color,
                        custom_size: Some(halo_size),
                        ..default()
                    },
                    // Render behind the drop shadow (shadow is at local z = -0.1).
                    Transform::from_xyz(0.0, 0.0, -0.25),
                    ClusterBlob,
                ));
            });
    }
}
