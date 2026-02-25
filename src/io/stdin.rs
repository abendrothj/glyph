use bevy::prelude::*;

#[derive(Resource)]
pub struct StdinSnapshot(pub crate::io::file_io::CanvasSnapshot);

pub fn load_stdin_snapshot_system(mut commands: Commands, snapshot: Option<Res<StdinSnapshot>>) {
    let Some(snap) = snapshot else {
        return;
    };
    let mut id_to_entity = std::collections::HashMap::new();

    for node in &snap.0.nodes {
        let color = node.color.to_bevy();
        let entity =
            crate::core::helpers::spawn_node_with_color(&mut commands, node.x, node.y, &node.text, color);
        id_to_entity.insert(node.id, entity);
    }

    for edge in &snap.0.edges {
        let Some(&source) = id_to_entity.get(&edge.source_id) else {
            continue;
        };
        let Some(&target) = id_to_entity.get(&edge.target_id) else {
            continue;
        };
        commands.spawn(crate::core::components::Edge {
            source,
            target,
            label: edge.label.clone(),
        });
    }
}
