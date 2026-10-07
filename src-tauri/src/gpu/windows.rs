use super::{is_real_adapter, pdh_names, AdapterInfo, GpuProbe, GpuReading};
use std::collections::HashMap;
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE,
};
use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
    PdhOpenQueryW, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_LARGE, PDH_HCOUNTER, PDH_HQUERY,
    PDH_MORE_DATA,
};

const ADAPTER_USED: &str = r"\GPU Adapter Memory(*)\Dedicated Usage";
const PROCESS_DEDICATED: &str = r"\GPU Process Memory(*)\Dedicated Usage";
const PROCESS_SHARED: &str = r"\GPU Process Memory(*)\Shared Usage";

pub struct WindowsGpuProbe;

impl GpuProbe for WindowsGpuProbe {
    fn read(&self) -> Result<GpuReading, String> {
        let adapters = enumerate_adapters()?;
        let totals: HashMap<u64, u64> = adapters.iter().map(|a| (a.luid, a.total_mb)).collect();
        let used = read_counter_array(ADAPTER_USED)?;
        let dedicated = read_counter_array(PROCESS_DEDICATED)?;
        let shared = read_counter_array(PROCESS_SHARED)?;
        Ok(GpuReading {
            adapter_used_mb: pdh_names::adapter_usage(&used),
            processes: pdh_names::aggregate_processes(&dedicated, &shared, &totals),
            adapters,
        })
    }
}

fn enumerate_adapters() -> Result<Vec<AdapterInfo>, String> {
    let factory: IDXGIFactory1 =
        unsafe { CreateDXGIFactory1() }.map_err(|e| format!("DXGI: {e}"))?;
    let mut adapters = Vec::new();
    let mut index = 0;
    while let Ok(adapter) = unsafe { factory.EnumAdapters1(index) } {
        index += 1;
        let desc = unsafe { adapter.GetDesc1() }.map_err(|e| format!("DXGI GetDesc1: {e}"))?;
        let name = String::from_utf16_lossy(&desc.Description)
            .trim_end_matches('\0')
            .to_string();
        let total_mb = desc.DedicatedVideoMemory as u64 / (1024 * 1024);
        let is_software = desc.Flags & (DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0;
        if is_real_adapter(&name, total_mb, is_software) {
            let luid = pdh_names::luid_key(desc.AdapterLuid.HighPart, desc.AdapterLuid.LowPart);
            adapters.push(AdapterInfo {
                luid,
                name,
                total_mb,
            });
        }
    }
    Ok(adapters)
}

fn read_counter_array(path: &str) -> Result<Vec<(String, i64)>, String> {
    let mut query = PDH_HQUERY::default();
    check(
        unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut query) },
        "PdhOpenQueryW",
    )?;
    let result = unsafe { collect_counter(query, path) };
    unsafe { PdhCloseQuery(query) };
    result
}

unsafe fn collect_counter(query: PDH_HQUERY, path: &str) -> Result<Vec<(String, i64)>, String> {
    let mut counter = PDH_HCOUNTER::default();
    check(
        PdhAddEnglishCounterW(query, &HSTRING::from(path), 0, &mut counter),
        "PdhAddEnglishCounterW",
    )?;
    check(PdhCollectQueryData(query), "PdhCollectQueryData")?;
    let (mut size, mut count) = (0u32, 0u32);
    let status = PdhGetFormattedCounterArrayW(counter, PDH_FMT_LARGE, &mut size, &mut count, None);
    if status == 0 {
        return Ok(Vec::new());
    }
    if status != PDH_MORE_DATA {
        return Err(format!("PdhGetFormattedCounterArrayW: 0x{status:08x}"));
    }
    // u64 asegura la alineación de 8 bytes de PDH_FMT_COUNTERVALUE_ITEM_W
    let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
    let items = buffer.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
    check(
        PdhGetFormattedCounterArrayW(counter, PDH_FMT_LARGE, &mut size, &mut count, Some(items)),
        "PdhGetFormattedCounterArrayW",
    )?;
    let slice = std::slice::from_raw_parts(items, count as usize);
    Ok(slice
        .iter()
        .filter(|item| pdh_names::is_valid_sample(item.FmtValue.CStatus))
        .map(|item| {
            (
                item.szName.to_string().unwrap_or_default(),
                item.FmtValue.Anonymous.largeValue,
            )
        })
        .collect())
}

fn check(status: u32, what: &str) -> Result<(), String> {
    if status == 0 {
        Ok(())
    } else {
        Err(format!("{what}: 0x{status:08x}"))
    }
}
