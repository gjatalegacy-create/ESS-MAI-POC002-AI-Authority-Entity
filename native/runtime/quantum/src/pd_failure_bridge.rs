//! native transport entry for d-bab witnessed missing-pair inquiry.
use shadow_contracts::pd_memory::{self as pd, failure};
use std::{io::Read, path::Path};
pub fn dispatch(args: &[String]) -> Option<i32> {
    if args.get(1).map(String::as_str) != Some("--pd-failure-once") {
        return None;
    }
    if args.len() != 4 {
        return Some(64);
    }
    let result = (|| {
        let mut bytes = Vec::new();
        std::fs::File::open(&args[2])
            .map_err(|e| e.to_string())?
            .take((pd::MAX_SOURCE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        let record = if pd::iz_science::source_is_science(&bytes) {
            let input = pd::iz_science::decode_input(&bytes)?;
            crate::pd_iz_science::continue_native(&input)?
        } else if bytes.starts_with(b"PDS1") {
            let input = pd::linked_science::decode_linked_input(&bytes)?;
            crate::pd_failure::continue_linked_native(&input)?
        } else {
            let input = failure::decode_failure_input(&bytes)?;
            crate::pd_failure::continue_native(&input)?
        };
        crate::pd_operational::publish_new(Path::new(&args[3]), &pd::encode_record(&record)?)
    })();
    Some(match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("[PD/FAILURE] {e}");
            70
        }
    })
}
