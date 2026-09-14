//! GPU/DMA live monitor acceptance entry point.
//!
//! The old example consumed the disabled CPU/RGBA preview path. Until a real
//! platform GPU/DMA backend exists, this example must fail clearly instead of
//! exercising a fallback.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend = qnc_player_frame_transport::preferred_dma_backend_for_current_os()
        .map(|backend| backend.wire_name())
        .unwrap_or("none");
    if qnc_player_frame_transport::active_gpu_dma_monitor_available() {
        println!("GPU/DMA monitor backend active: {backend}");
        return Ok(());
    }

    Err(format!(
        "GPU/DMA monitor backend is required (preferred backend: {backend}); CPU/RGBA live monitor fallback is disabled",
    )
    .into())
}
