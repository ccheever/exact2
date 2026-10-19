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
/// Familiar names and their shipped API spellings.
#[doc = include_str!("../../names.md")]
pub mod names {}

/// Draw rings and batched translucent halos.
#[doc = include_str!("../../recipes/halo.md")]
pub mod halo {}
/// Spawn a prop or sprite from literal values.
#[doc = include_str!("../../recipes/literal-builders.md")]
pub mod literal_builders {}
/// Bubbles, a wake and a splash from emitters.
#[doc = include_str!("../../recipes/emitter.md")]
pub mod emitter {}
/// Name synthesized sounds and play them from the tick.
#[doc = include_str!("../../recipes/synthesized-sounds.md")]
pub mod synthesized_sounds {}
/// Orbit a third-person camera around the player.
#[doc = include_str!("../../recipes/orbit-camera.md")]
pub mod orbit_camera {}
/// Flash a hit entity from `Game::present`.
#[doc = include_str!("../../recipes/hit-flash.md")]
pub mod hit_flash {}
/// Read a tap whose down and up land inside one tick.
#[doc = include_str!("../../recipes/same-tick-press.md")]
pub mod same_tick_press {}
/// Export models from Blender for the bake.
#[doc = include_str!("../../recipes/blender-export.md")]
pub mod blender_export {}
/// Light and frame a scene under water.
#[doc = include_str!("../../recipes/underwater.md")]
pub mod underwater {}
