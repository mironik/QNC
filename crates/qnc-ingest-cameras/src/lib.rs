//! The cameras Ingest can read. This is the one place that names them.

use qnc_camera_adapter::CameraRegistry;
use std::sync::Arc;

pub const MODULE_ID: &str = "qnc.module.ingest-cameras";
pub const VERSION: &str = "0.1.0";

/// Registry of every camera adapter Ingest composes.
pub fn registry() -> Result<CameraRegistry, String> {
    let mut registry = CameraRegistry::new();
    registry.register(Arc::new(qnc_camera_sony_fx6_v6::SonyFx6V6::new()))?;
    Ok(registry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_has_the_sony_fx6_adapter() {
        let registry = registry().unwrap();
        assert!(!registry.is_empty());
        assert!(registry.for_reader("camera.sony.index.read").is_some());
    }
}
