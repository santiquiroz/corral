use super::ProcessGpuMem;
use std::collections::HashMap;

const MIB: u64 = 1024 * 1024;

pub fn parse_adapter_instance(instance: &str) -> Option<u64> {
    parse_luid_parts(instance.strip_prefix("luid_")?)
}

pub fn parse_process_instance(instance: &str) -> Option<(u32, u64)> {
    let rest = instance.strip_prefix("pid_")?;
    let (pid, luid) = rest.split_once("_luid_")?;
    Some((pid.parse().ok()?, parse_luid_parts(luid)?))
}

fn parse_luid_parts(rest: &str) -> Option<u64> {
    let mut parts = rest.split('_');
    let high = u32::from_str_radix(parts.next()?.strip_prefix("0x")?, 16).ok()?;
    let low = u32::from_str_radix(parts.next()?.strip_prefix("0x")?, 16).ok()?;
    Some(((high as u64) << 32) | low as u64)
}

pub fn luid_key(high: i32, low: u32) -> u64 {
    ((high as u32 as u64) << 32) | low as u64
}

pub fn bytes_to_mb(bytes: i64) -> u64 {
    if bytes <= 0 {
        0
    } else {
        bytes as u64 / MIB
    }
}

pub fn sanitize_mb(value_mb: u64, total_mb: u64) -> Option<u64> {
    if total_mb > 0 && value_mb > total_mb {
        None
    } else {
        Some(value_mb)
    }
}

pub fn adapter_usage(samples: &[(String, i64)]) -> HashMap<u64, u64> {
    let mut usage = HashMap::new();
    for (instance, bytes) in samples {
        if let Some(luid) = parse_adapter_instance(instance) {
            *usage.entry(luid).or_insert(0) += bytes_to_mb(*bytes);
        }
    }
    usage
}

pub fn aggregate_processes(
    dedicated: &[(String, i64)],
    shared: &[(String, i64)],
    totals: &HashMap<u64, u64>,
) -> Vec<ProcessGpuMem> {
    let dedicated = sum_by_process(dedicated);
    let shared = sum_by_process(shared);
    dedicated
        .iter()
        .map(|(&(pid, luid), &mb)| ProcessGpuMem {
            pid,
            luid,
            dedicated_mb: sanitize_mb(mb, totals.get(&luid).copied().unwrap_or(0)),
            shared_mb: Some(shared.get(&(pid, luid)).copied().unwrap_or(0)),
        })
        .collect()
}

fn sum_by_process(samples: &[(String, i64)]) -> HashMap<(u32, u64), u64> {
    let mut sums = HashMap::new();
    for (instance, bytes) in samples {
        if let Some(key) = parse_process_instance(instance) {
            *sums.entry(key).or_insert(0) += bytes_to_mb(*bytes);
        }
    }
    sums
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIB: i64 = 1024 * 1024;

    #[test]
    fn parses_adapter_and_process_instances() {
        assert_eq!(
            parse_adapter_instance("luid_0x00000000_0x0001685a_phys_0"),
            Some(0x1685a)
        );
        assert_eq!(
            parse_adapter_instance("luid_0x00000001_0x00000002_phys_0"),
            Some((1u64 << 32) | 2)
        );
        assert_eq!(
            parse_process_instance("pid_59380_luid_0x00000000_0x0001685a_phys_0"),
            Some((59380, 0x1685a))
        );
        assert_eq!(parse_process_instance("_Total"), None);
        assert_eq!(parse_process_instance("pid_x_luid_0x0_0x1_phys_0"), None);
        assert_eq!(parse_adapter_instance("luid_zz_0x1_phys_0"), None);
    }

    #[test]
    fn luid_key_matches_instance_encoding() {
        assert_eq!(
            luid_key(0, 0x1685a),
            parse_adapter_instance("luid_0x00000000_0x0001685a_phys_0").unwrap()
        );
        assert_eq!(luid_key(-1, 0), 0xffff_ffff_0000_0000);
    }

    #[test]
    fn absurd_values_are_discarded() {
        assert_eq!(sanitize_mb(7819, 16368), Some(7819));
        assert_eq!(sanitize_mb(67_481, 16368), None);
        assert_eq!(sanitize_mb(500, 0), Some(500));
        assert_eq!(bytes_to_mb(-5), 0);
    }

    #[test]
    fn aggregates_by_pid_and_luid_and_sanitizes_dedicated() {
        let totals = HashMap::from([(0x1685a, 16368)]);
        let dedicated = vec![
            (
                "pid_59380_luid_0x00000000_0x0001685a_phys_0".to_string(),
                7819 * MIB,
            ),
            (
                "pid_17272_luid_0x00000000_0x0001685a_phys_0".to_string(),
                67_481 * MIB,
            ),
            ("basura".to_string(), 1),
        ];
        let shared = vec![(
            "pid_59380_luid_0x00000000_0x0001685a_phys_0".to_string(),
            729 * MIB,
        )];
        let mut rows = aggregate_processes(&dedicated, &shared, &totals);
        rows.sort_by_key(|r| r.pid);
        assert_eq!(
            rows,
            vec![
                ProcessGpuMem {
                    pid: 17272,
                    luid: 0x1685a,
                    dedicated_mb: None,
                    shared_mb: Some(0)
                },
                ProcessGpuMem {
                    pid: 59380,
                    luid: 0x1685a,
                    dedicated_mb: Some(7819),
                    shared_mb: Some(729)
                },
            ]
        );
    }

    #[test]
    fn adapter_usage_sums_phys_entries() {
        let samples = vec![
            (
                "luid_0x00000000_0x0001685a_phys_0".to_string(),
                14_000 * MIB,
            ),
            ("luid_0x00000000_0x00018ca0_phys_0".to_string(), 309 * MIB),
        ];
        assert_eq!(
            adapter_usage(&samples),
            HashMap::from([(0x1685a, 14_000), (0x18ca0, 309)])
        );
    }
}
