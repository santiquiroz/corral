pub mod pdh_names;
#[cfg(windows)]
pub mod windows;

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct AdapterInfo {
    pub luid: u64,
    pub name: String,
    pub total_mb: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProcessGpuMem {
    pub pid: u32,
    pub luid: u64,
    pub dedicated_mb: Option<u64>,
    pub shared_mb: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GpuReading {
    pub adapters: Vec<AdapterInfo>,
    pub adapter_used_mb: HashMap<u64, u64>,
    pub processes: Vec<ProcessGpuMem>,
}

pub trait GpuProbe: Send + Sync {
    fn read(&self) -> Result<GpuReading, String>;
}

pub struct UnsupportedGpuProbe;

impl GpuProbe for UnsupportedGpuProbe {
    fn read(&self) -> Result<GpuReading, String> {
        Err("telemetría de GPU no disponible en esta plataforma todavía".into())
    }
}

pub fn default_probe() -> Box<dyn GpuProbe> {
    #[cfg(windows)]
    {
        Box::new(windows::WindowsGpuProbe)
    }
    #[cfg(not(windows))]
    {
        Box::new(UnsupportedGpuProbe)
    }
}

pub fn is_real_adapter(name: &str, total_mb: u64, is_software: bool) -> bool {
    !is_software && total_mb > 0 && !name.contains("IddSampleDriver")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_virtual_and_software_adapters() {
        assert!(is_real_adapter("AMD Radeon RX 7800 XT", 16368, false));
        assert!(is_real_adapter("AMD Radeon(TM) Graphics", 512, false));
        assert!(!is_real_adapter("IddSampleDriver Device", 0, false));
        assert!(!is_real_adapter("Microsoft Basic Render Driver", 0, true));
    }
}
