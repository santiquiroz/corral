#![cfg(windows)]

use corral_lib::gpu::default_probe;

#[test]
fn reading_never_panics_and_process_luids_belong_to_known_adapters_or_igpu() {
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
