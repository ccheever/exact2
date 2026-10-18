//! Runnable game recipes. Their code blocks run with `cargo test -p exact-game --doc`.

/// Follow a player with a camera.
#[doc = include_str!("../../recipes/follow-camera.md")]
pub mod follow_camera {}
/// Select a pickup and publish its prompt.
#[doc = include_str!("../../recipes/pickup.md")]
pub mod pickup {}
/// Publish a countdown from saved ticks.
#[doc = include_str!("../../recipes/timer-hud.md")]
pub mod timer_hud {}
/// Turn a camera from device deltas.
#[doc = include_str!("../../recipes/mouse-look.md")]
pub mod mouse_look {}
/// Play once on a landing edge.
#[doc = include_str!("../../recipes/sound-on-contact.md")]
pub mod sound_on_contact {}
/// Spawn on the simulation clock.
#[doc = include_str!("../../recipes/spawner.md")]
pub mod spawner {}
/// Receive a command from Contract.
#[doc = include_str!("../../recipes/door-message.md")]
pub mod door_message {}
/// Save and restore an opaque slot.
#[doc = include_str!("../../recipes/save-slot.md")]
pub mod save_slot {}
/// Inspect defaults before spawning a component.
#[doc = include_str!("../../recipes/component-values.md")]
pub mod component_values {}
