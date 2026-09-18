#[cfg(all(not(test), any(target_os = "macos", target_os = "ios")))]
use crate::AppleOutput as Device;
#[cfg(all(not(test), target_arch = "wasm32"))]
use crate::WebOutput as Device;
#[cfg(any(test, target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
use crate::{Listener, Output};
use crate::{NullOutput, Player, Transport};
use exact_game::World;

/// Lazy audio owner for the renderer's dependency-free presentation hook.
/// Default construction and every seekable/headless frame open no device.
pub struct SurfacePlayer {
    null: Player<NullOutput>,
    #[cfg(any(test, target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
    device: Option<Player<Device>>,
    #[cfg(any(test, target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
    retry_frames: u32,
    #[cfg(any(test, target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
    warned: bool,
    seekable: bool,
    suspended: bool,
    unlocked: bool,
    epoch: u64,
}
impl Default for SurfacePlayer {
    fn default() -> Self {
        Self {
            null: Player::new(NullOutput, 48000),
            #[cfg(any(test, target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
            device: None,
            #[cfg(any(test, target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
            retry_frames: 0,
            #[cfg(any(test, target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
            warned: false,
            seekable: true,
            suspended: false,
            unlocked: false,
            epoch: 0,
        }
    }
}
impl SurfacePlayer {
    /// Set clock ownership before input; changing to an agent clock closes devices.
    pub fn clock(&mut self, seekable: bool) {
        self.seekable = seekable;
        if seekable {
            #[cfg(any(test, target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
            {
                self.device = None;
            }
            self.unlocked = false;
        }
    }
    /// Aggregate visibility/interruption state. Repeated notifications are no-ops.
    pub fn suspend(&mut self, suspended: bool) {
        if self.suspended == suspended {
            return;
        }
        self.suspended = suspended;
        if suspended {
            self.unlocked = false;
        }
        #[cfg(any(test, target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
        if let Some(player) = &mut self.device {
            if suspended {
                let _ = player.output.suspend();
            } else {
                let _ = player.output.resume();
            }
        }
    }
    #[cfg(any(test, target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
    fn ensure_device(&mut self) {
        if self.seekable || self.suspended || self.device.is_some() || self.retry_frames != 0 {
            return;
        }
        match Device::new() {
            Ok(output) => self.device = Some(Player::new(output, 48000)),
            Err(error) => {
                // Five seconds of frames at 60 Hz; input cannot defeat the bound.
                self.retry_frames = 300;
                if !self.warned {
                    eprintln!("game audio unavailable (retrying every 300 live frames): {error:?}");
                    self.warned = true;
                }
            }
        }
    }

    pub fn sync(&mut self, world: &World, generation: u64, playing: bool, seekable: bool) {
        self.clock(seekable);
        #[cfg(any(test, target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
        {
            if !seekable && !self.suspended {
                self.retry_frames = self.retry_frames.saturating_sub(1);
                self.ensure_device();
                if let Some(player) = &mut self.device {
                    let ready = player.output.ready();
                    if ready && !self.unlocked {
                        self.epoch = self.epoch.wrapping_add(1);
                    }
                    self.unlocked = ready;
                    player.sync(
                        world,
                        Listener::from_world(world),
                        Transport {
                            generation: generation.wrapping_add(self.epoch),
                            playing,
                        },
                    );
                    return;
                }
            }
        }
        let _ = (seekable, self.unlocked);
        self.null.sync(
            world,
            None,
            Transport {
                generation: generation.wrapping_add(self.epoch),
                playing,
            },
        );
    }
    pub fn unlock(&mut self) {
        #[cfg(any(test, target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
        {
            if self.seekable || self.suspended {
                return;
            }
            self.ensure_device();
            if let Some(player) = &mut self.device {
                Output::unlock(&mut player.output);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seekable_gestures_never_construct_a_device_or_pcm() {
        test_device::ATTEMPTS.set(0);
        test_device::SUSPENDS.set(0);
        test_device::RESUMES.set(0);
        let mut surface = SurfacePlayer::default();
        let mut world = World::new(60, 0);
        world.register_audio();
        world
            .resource_mut::<exact_game::audio::Sounds>()
            .add("tone", exact_game::audio::Synth::sine(440.));
        world.play("tone").start();
        surface.unlock();
        for generation in 0..10 {
            surface.sync(&world, generation, true, true);
            surface.suspend(true);
            surface.suspend(false);
            surface.unlock();
        }
        assert_eq!(surface.null.cached_sounds(), 0);
        assert_eq!(test_device::ATTEMPTS.get(), 0);
        assert_eq!(test_device::SUSPENDS.get(), 0);
        assert_eq!(test_device::RESUMES.get(), 0);
        #[cfg(any(test, target_arch = "wasm32", target_os = "macos", target_os = "ios"))]
        assert!(surface.device.is_none() && surface.retry_frames == 0);
    }
}

#[cfg(test)]
use test_device::Device;
#[cfg(test)]
mod test_device {
    use crate::Output;
    use std::{cell::Cell, sync::Arc};
    std::thread_local! {
        pub static ATTEMPTS: Cell<usize> = const { Cell::new(0) };
        pub static FAIL: Cell<bool> = const { Cell::new(false) };
        pub static UNLOCKS: Cell<usize> = const { Cell::new(0) };
        pub static SUSPENDS: Cell<usize> = const { Cell::new(0) };
        pub static RESUMES: Cell<usize> = const { Cell::new(0) };
    }
    pub struct Device {
        suspended: bool,
    }
    impl Device {
        pub fn new() -> Result<Self, &'static str> {
            ATTEMPTS.set(ATTEMPTS.get() + 1);
            if FAIL.get() {
                Err("test output unavailable")
            } else {
                Ok(Self { suspended: false })
            }
        }
    }
    impl Device {
        pub fn suspend(&mut self) -> Result<(), ()> {
            SUSPENDS.set(SUSPENDS.get() + 1);
            self.suspended = true;
            Ok(())
        }
        pub fn resume(&mut self) -> Result<(), ()> {
            RESUMES.set(RESUMES.get() + 1);
            self.suspended = false;
            Ok(())
        }
    }
    impl Output for Device {
        fn ready(&self) -> bool {
            !self.suspended
        }
        fn unlock(&mut self) {
            UNLOCKS.set(UNLOCKS.get() + 1);
        }
        fn start_at(&mut self, _: u64, _: &Arc<[f32]>, _: u32, _: bool, _: usize, _: f32) {}
        fn set(&mut self, _: u64, _: f32, _: f32) {}
        fn stop(&mut self, _: u64) {}
    }
    #[test]
    fn seekable_frame_preserves_failed_device_cooldown() {
        FAIL.set(true);
        ATTEMPTS.set(0);
        let mut surface = super::SurfacePlayer::default();
        let world = exact_game::World::new(60, 0);
        surface.sync(&world, 0, true, false);
        surface.sync(&world, 0, true, true);
        surface.sync(&world, 0, true, false);
        assert_eq!(ATTEMPTS.get(), 1);
        assert_eq!(surface.retry_frames, 299);
        FAIL.set(false);
    }
    #[test]
    fn first_live_gesture_constructs_and_unlocks_before_any_frame() {
        FAIL.set(false);
        ATTEMPTS.set(0);
        UNLOCKS.set(0);
        let mut surface = super::SurfacePlayer::default();
        surface.clock(false);
        surface.unlock();
        assert_eq!(ATTEMPTS.get(), 1);
        assert_eq!(UNLOCKS.get(), 1);
        surface.clock(true);
        surface.unlock();
        assert!(surface.device.is_none());
        assert_eq!(ATTEMPTS.get(), 1);
    }
    #[test]
    fn suspension_resumes_once_and_never_changes_world() {
        FAIL.set(false);
        SUSPENDS.set(0);
        RESUMES.set(0);
        let mut surface = super::SurfacePlayer::default();
        let world = exact_game::World::new(60, 0);
        let before = world.save();
        surface.sync(&world, 0, true, false);
        let epoch = surface.epoch;
        surface.suspend(false);
        surface.sync(&world, 0, true, false);
        assert_eq!(surface.epoch, epoch);
        assert_eq!(RESUMES.get(), 0);
        surface.suspend(true);
        surface.suspend(true);
        surface.sync(&world, 0, true, false);
        surface.suspend(false);
        surface.sync(&world, 0, true, false);
        assert_eq!(surface.epoch, epoch + 1);
        surface.suspend(false);
        surface.sync(&world, 0, true, false);
        assert_eq!(surface.epoch, epoch + 1);
        assert_eq!(world.save(), before);
        assert_eq!(SUSPENDS.get(), 1);
        assert_eq!(RESUMES.get(), 1);
    }
    #[test]
    fn failed_device_creation_retries_at_bounded_frame_intervals() {
        FAIL.set(true);
        ATTEMPTS.set(0);
        let mut surface = super::SurfacePlayer::default();
        let world = exact_game::World::new(60, 0);
        for _ in 0..601 {
            surface.sync(&world, 0, true, false);
        }
        assert_eq!(ATTEMPTS.get(), 3);
        FAIL.set(false);
        for _ in 0..300 {
            surface.sync(&world, 0, true, false);
        }
        assert_eq!(ATTEMPTS.get(), 4);
        assert!(surface.device.is_some());
        for _ in 0..600 {
            surface.sync(&world, 0, true, false);
        }
        assert_eq!(ATTEMPTS.get(), 4);
    }
}
