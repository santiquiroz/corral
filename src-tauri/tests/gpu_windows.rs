#![cfg(windows)]

use corral_lib::gpu::default_probe;

#[test]
fn reading_never_panics_and_reported_adapters_have_vram() {
    let probe = default_probe();
    match probe.read() {
        Ok(reading) => {
            for process in &reading.processes {
                assert!(process.pid > 0);
            }
            for adapter in &reading.adapters {
                assert!(
                    adapter.total_mb > 0,
                    "adaptador sin VRAM no filtrado: {}",
                    adapter.name
                );
            }
        }
        Err(reason) => assert!(!reason.is_empty()),
    }
}
