use bevy::prelude::*;
use std::collections::HashMap;

/// Resource storing saved marks mapped from a character ('a'..='z') to a world position
#[derive(Resource, Default)]
pub struct Marks {
    pub locations: HashMap<char, Vec2>,
}

/// Helper to set a mark at a position
pub fn set_mark(marks: &mut Marks, key: char, pos: Vec2) {
    marks.locations.insert(key, pos);
}

/// Helper to get a mark position
pub fn get_mark(marks: &Marks, key: char) -> Option<Vec2> {
    marks.locations.get(&key).copied()
}
