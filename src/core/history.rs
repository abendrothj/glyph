use crate::core::components::{Edge, MainCamera, NodeColor, Selected, SourceLocation, TextData};
use crate::core::helpers::spawn_canvas_node;
use bevy::prelude::*;

/// Represents a reversible action in the whiteboard.
#[derive(Clone, Debug)]
pub enum Action {
    CreateNode {
        entity: Entity,
        pos: Vec2,
        text: String,
        color: Color,
    },
    DeleteNode {
        pos: Vec2,
        text: String,
        color: Color,
        // (source_entity, target_entity, label)
        edges: Vec<(Entity, Entity, Option<String>)>,
    },
    MoveNode {
        entity: Entity,
        from: Vec2,
        to: Vec2,
    },
    EditText {
        entity: Entity,
        old: String,
        new: String,
    },
    CreateEdge {
        entity: Entity,
        source: Entity,
        target: Entity,
        label: Option<String>,
    },
    DeleteEdge {
        source: Entity,
        target: Entity,
        label: Option<String>,
    },
}

#[derive(Resource, Default)]
pub struct UndoHistory {
    pub undo_stack: Vec<Action>,
    pub redo_stack: Vec<Action>,
}

impl UndoHistory {
    pub fn push(&mut self, action: Action) {
        self.undo_stack.push(action);
        self.redo_stack.clear();
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
    }

    pub fn pop_undo(&mut self) -> Option<Action> {
        self.undo_stack.pop()
    }

    pub fn pop_redo(&mut self) -> Option<Action> {
        self.redo_stack.pop()
    }

    pub fn push_redo(&mut self, action: Action) {
        self.redo_stack.push(action);
    }
}

pub fn apply_action(
    action: &Action,
    revert: bool,
    commands: &mut Commands,
    query: &mut Query<
        (
            Entity,
            &mut Transform,
            &mut TextData,
            &mut NodeColor,
            Option<&SourceLocation>,
        ),
        (With<Selected>, Without<MainCamera>),
    >,
    edge_query: &Query<(Entity, &Edge)>,
) {
    match action {
        Action::CreateNode {
            pos, text, color, ..
        } => {
            if revert {
                if let Some((e, ..)) = query
                    .iter()
                    .find(|(_, t, ..)| (t.translation.truncate() - *pos).length() < 0.1)
                {
                    commands.entity(e).despawn();
                }
            } else {
                spawn_canvas_node(commands, *pos, text, *color, false);
            }
        }
        Action::DeleteNode {
            pos, text, color, ..
        } => {
            if revert {
                spawn_canvas_node(commands, *pos, text, *color, false);
            } else {
                if let Some((e, ..)) = query
                    .iter()
                    .find(|(_, t, ..)| (t.translation.truncate() - *pos).length() < 0.1)
                {
                    commands.entity(e).despawn();
                }
            }
        }
        Action::MoveNode { entity, from, to } => {
            let target_pos = if revert { *from } else { *to };
            if let Ok((_, mut transform, ..)) = query.get_mut(*entity) {
                transform.translation.x = target_pos.x;
                transform.translation.y = target_pos.y;
            }
        }
        Action::EditText { entity, old, new } => {
            let target_text = if revert { old } else { new };
            if let Ok((_, _, mut text_data, ..)) = query.get_mut(*entity) {
                text_data.content = target_text.clone();
            }
        }
        Action::CreateEdge {
            entity,
            source,
            target,
            label,
        } => {
            if revert {
                if let Ok(mut e_cmd) = commands.get_entity(*entity) {
                    e_cmd.despawn();
                }
            } else {
                commands.spawn(Edge {
                    source: *source,
                    target: *target,
                    label: label.clone(),
                });
            }
        }
        Action::DeleteEdge {
            source,
            target,
            label,
        } => {
            if revert {
                commands.spawn(Edge {
                    source: *source,
                    target: *target,
                    label: label.clone(),
                });
            } else {
                for (e, edge) in edge_query.iter() {
                    if edge.source == *source && edge.target == *target && edge.label == *label {
                        commands.entity(e).despawn();
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::world::World;

    fn test_entity(world: &mut World) -> Entity {
        world.spawn_empty().id()
    }

    fn make_move_action(entity: Entity, id: f32) -> Action {
        Action::MoveNode {
            entity,
            from: Vec2::ZERO,
            to: Vec2::new(id, 0.0),
        }
    }

    #[test]
    fn push_adds_to_undo_stack() {
        let mut world = World::new();
        let e = test_entity(&mut world);
        let mut h = UndoHistory::default();
        assert!(h.undo_stack.is_empty());

        h.push(make_move_action(e, 1.0));
        assert_eq!(h.undo_stack.len(), 1);

        h.push(make_move_action(e, 2.0));
        assert_eq!(h.undo_stack.len(), 2);
    }

    #[test]
    fn push_clears_redo_stack() {
        let mut world = World::new();
        let e = test_entity(&mut world);
        let mut h = UndoHistory::default();
        h.push_redo(make_move_action(e, 10.0));
        h.push_redo(make_move_action(e, 11.0));
        assert_eq!(h.redo_stack.len(), 2);

        h.push(make_move_action(e, 1.0));
        assert!(
            h.redo_stack.is_empty(),
            "redo stack should be cleared on new push"
        );
    }

    #[test]
    fn pop_undo_returns_last_pushed() {
        let mut world = World::new();
        let e = test_entity(&mut world);
        let mut h = UndoHistory::default();
        h.push(make_move_action(e, 1.0));
        h.push(make_move_action(e, 2.0));

        let action = h.pop_undo().unwrap();
        match action {
            Action::MoveNode { to, .. } => assert_eq!(to.x, 2.0),
            _ => panic!("expected MoveNode"),
        }
        assert_eq!(h.undo_stack.len(), 1);
    }

    #[test]
    fn pop_undo_returns_none_when_empty() {
        let mut h = UndoHistory::default();
        assert!(h.pop_undo().is_none());
    }

    #[test]
    fn pop_redo_returns_last_pushed() {
        let mut world = World::new();
        let e = test_entity(&mut world);
        let mut h = UndoHistory::default();
        h.push_redo(make_move_action(e, 5.0));
        h.push_redo(make_move_action(e, 6.0));

        let action = h.pop_redo().unwrap();
        match action {
            Action::MoveNode { to, .. } => assert_eq!(to.x, 6.0),
            _ => panic!("expected MoveNode"),
        }
        assert_eq!(h.redo_stack.len(), 1);
    }

    #[test]
    fn pop_redo_returns_none_when_empty() {
        let mut h = UndoHistory::default();
        assert!(h.pop_redo().is_none());
    }

    #[test]
    fn undo_stack_capped_at_100() {
        let mut world = World::new();
        let e = test_entity(&mut world);
        let mut h = UndoHistory::default();
        for i in 0..110 {
            h.push(make_move_action(e, i as f32));
        }
        assert_eq!(h.undo_stack.len(), 100);
        // The oldest actions should have been removed
        match &h.undo_stack[0] {
            Action::MoveNode { to, .. } => assert_eq!(to.x, 10.0),
            _ => panic!("expected MoveNode"),
        }
    }

    #[test]
    fn full_undo_redo_cycle() {
        let mut world = World::new();
        let e = test_entity(&mut world);
        let mut h = UndoHistory::default();
        h.push(make_move_action(e, 1.0));
        h.push(make_move_action(e, 2.0));
        h.push(make_move_action(e, 3.0));

        // Undo last two
        let a3 = h.pop_undo().unwrap();
        h.push_redo(a3);
        let a2 = h.pop_undo().unwrap();
        h.push_redo(a2);

        assert_eq!(h.undo_stack.len(), 1);
        assert_eq!(h.redo_stack.len(), 2);

        // Redo one
        let redo = h.pop_redo().unwrap();
        h.undo_stack.push(redo);
        assert_eq!(h.undo_stack.len(), 2);
        assert_eq!(h.redo_stack.len(), 1);
    }

    #[test]
    fn new_action_after_undo_clears_redo() {
        let mut world = World::new();
        let e = test_entity(&mut world);
        let mut h = UndoHistory::default();
        h.push(make_move_action(e, 1.0));
        h.push(make_move_action(e, 2.0));

        let a = h.pop_undo().unwrap();
        h.push_redo(a);
        assert_eq!(h.redo_stack.len(), 1);

        // New action should clear redo
        h.push(make_move_action(e, 99.0));
        assert!(h.redo_stack.is_empty());
        assert_eq!(h.undo_stack.len(), 2);
    }

    #[test]
    fn action_create_node_clone() {
        let mut world = World::new();
        let e = test_entity(&mut world);
        let action = Action::CreateNode {
            entity: e,
            pos: Vec2::new(10.0, 20.0),
            text: "hello".to_string(),
            color: Color::srgb(1.0, 0.0, 0.0),
        };
        let cloned = action.clone();
        match cloned {
            Action::CreateNode { text, pos, .. } => {
                assert_eq!(text, "hello");
                assert_eq!(pos, Vec2::new(10.0, 20.0));
            }
            _ => panic!("expected CreateNode"),
        }
    }

    #[test]
    fn action_delete_node_with_edges() {
        let mut world = World::new();
        let e1 = test_entity(&mut world);
        let e2 = test_entity(&mut world);
        let e3 = test_entity(&mut world);
        let e4 = test_entity(&mut world);
        let action = Action::DeleteNode {
            pos: Vec2::new(5.0, 5.0),
            text: "node".to_string(),
            color: Color::WHITE,
            edges: vec![(e1, e2, Some("edge1".into())), (e3, e4, None)],
        };
        match &action {
            Action::DeleteNode { edges, .. } => {
                assert_eq!(edges.len(), 2);
                assert_eq!(edges[0].2, Some("edge1".to_string()));
                assert!(edges[1].2.is_none());
            }
            _ => panic!("expected DeleteNode"),
        }
    }

    #[test]
    fn action_edit_text() {
        let mut world = World::new();
        let e = test_entity(&mut world);
        let action = Action::EditText {
            entity: e,
            old: "before".to_string(),
            new: "after".to_string(),
        };
        match &action {
            Action::EditText { old, new, .. } => {
                assert_eq!(old, "before");
                assert_eq!(new, "after");
            }
            _ => panic!("expected EditText"),
        }
    }

    #[test]
    fn action_edge_variants() {
        let mut world = World::new();
        let e = test_entity(&mut world);
        let e2 = test_entity(&mut world);
        let e3 = test_entity(&mut world);
        let create = Action::CreateEdge {
            entity: e,
            source: e2,
            target: e3,
            label: Some("calls".into()),
        };
        let delete = Action::DeleteEdge {
            source: e2,
            target: e3,
            label: None,
        };
        // Ensure they construct and clone without panic
        let _ = create.clone();
        let _ = delete.clone();
    }
}
