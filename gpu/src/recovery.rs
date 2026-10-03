use crate::{json, load_gpu, DeviceFailure, Module};
use std::sync::atomic::Ordering;

pub(crate) struct RecoveryFailure {
    code: &'static str,
    message: String,
}

impl RecoveryFailure {
    pub(crate) fn device(error: DeviceFailure) -> Self {
        Self {
            code: error.kind().code(),
            message: error.to_string(),
        }
    }

    pub(crate) fn other(message: impl Into<String>) -> Self {
        Self {
            code: "recovery",
            message: message.into(),
        }
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }

    pub(crate) fn json(&self) -> String {
        let quoted = json::strings(&[self.message.clone()]);
        format!(
            "{{\"status\":\"failed\",\"code\":\"{}\",\"error\":{}}}",
            self.code,
            &quoted[1..quoted.len() - 1]
        )
    }
}

#[cfg(test)]
mod tests {
    use super::RecoveryFailure;
    use crate::{DeviceFailure, DeviceFailureKind};

    #[test]
    fn device_failures_keep_their_typed_host_json() {
        let no_adapter = RecoveryFailure::device(DeviceFailure::new(
            DeviceFailureKind::NoAdapter,
            "requestAdapter returned null",
        ));
        assert_eq!(
            no_adapter.json(),
            r#"{"status":"failed","code":"no-adapter","error":"no adapter: requestAdapter returned null"}"#
        );

        let no_device = RecoveryFailure::device(DeviceFailure::new(
            DeviceFailureKind::NoDevice,
            "requestDevice rejected \"limits\"",
        ));
        assert_eq!(
            no_device.json(),
            r#"{"status":"failed","code":"no-device","error":"no device: requestDevice rejected \"limits\""}"#
        );
    }
}

impl Module {
    /// Recover only a lost device; presentation configuration survives every error.
    pub(crate) async fn recover(&mut self) -> Result<String, RecoveryFailure> {
        let Some(instance) = self.instance.clone() else {
            return Ok("{\"status\":\"no device\",\"instances\":[]}".into());
        };
        self.check_device();
        if self.gpu.is_some() {
            return Ok("{\"status\":\"healthy\",\"instances\":[]}".into());
        }
        // No adapter request may consult a surface owned by the lost device.
        let gpu = load_gpu(instance, None)
            .await
            .map_err(RecoveryFailure::device)?;
        self.set_gpu(gpu);
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        if let Err(error) = self.reattach_layers() {
            self.lose_device();
            return Err(RecoveryFailure::other(error));
        }
        if self.device_lost.load(Ordering::Acquire) {
            self.lose_device();
            return Err(RecoveryFailure::other(
                "replacement device was lost during recovery",
            ));
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
            inst.surface.device_ready(gpu.device.features());
            inst.surface
                .prepare_assets(&gpu.device, &gpu.queue, config.format);
            inst.presentation = Some(std::sync::Arc::new(target));
        }
        Ok(())
    }
}
