use crate::{load_gpu, Module};
use std::sync::atomic::Ordering;

impl Module {
    /// Recover only a lost device; presentation configuration survives every error.
    pub async fn recover(&mut self) -> Result<String, String> {
        let Some(instance) = self.instance.clone() else {
            return Ok("{\"status\":\"no device\",\"instances\":[]}".into());
        };
        self.check_device();
        if self.gpu.is_some() {
            return Ok("{\"status\":\"healthy\",\"instances\":[]}".into());
        }
        // No adapter request may consult a surface owned by the lost device.
        let gpu = load_gpu(instance, None).await?;
        self.set_gpu(gpu);
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        if let Err(error) = self.reattach_layers() {
            self.lose_device();
            return Err(error);
        }
        if self.device_lost.load(Ordering::Acquire) {
            self.lose_device();
            return Err("replacement device was lost during recovery".into());
        }
        Ok(self.recovery_report())
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn reattach_layers(&mut self) -> Result<(), String> {
        let gpu = self.gpu.as_ref().ok_or("no device")?;
        for inst in self.instances.values_mut() {
            let (Some(layer), Some(config)) = (inst.layer, inst.config.as_ref()) else {
                continue;
            };
            // SAFETY: native::create retains the caller's live-layer contract until destroy.
            let target = unsafe {
                gpu.instance
                    .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(
                        layer as *mut std::ffi::c_void,
                    ))
            }
            .map_err(|e| e.to_string())?;
            target.configure(&gpu.device, config);
            inst.surface.device_ready();
            inst.surface
                .prepare_assets(&gpu.device, &gpu.queue, config.format);
            inst.presentation = Some(target);
        }
        Ok(())
    }
}
