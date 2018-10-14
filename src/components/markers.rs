//! Components that act as markers for specific properties of entities

use specs::{NullStorage, HashMapStorage};

/// An entity that is unable to move until the given duration has elapsed
#[derive(Debug, Default, Component)]
#[storage(HashMapStorage)]
pub struct Wait {
    pub duration: usize, // frames
    pub frames_elapsed: usize, // frames
}

/// The keyboard controlled player. Only one entity should hold this at a given time.
#[derive(Debug, Default, Component)]
#[storage(NullStorage)]
pub struct KeyboardControlled;

/// The entity with this component and a Position component will be centered in the camera
/// when the scene is rendered.
/// Only one entity should hold this at a given time.
#[derive(Debug, Default, Component)]
#[storage(NullStorage)]
pub struct CameraFocus;
