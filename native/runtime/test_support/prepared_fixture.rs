// Test-only deterministic vectors: real PD checkpoints, synthetic receipts.
// Quantum regenerates and compares every byte; Shadow independently validates them.
pub fn bytes(kind: &str, file: &str) -> &'static [u8] {
    match (kind, file) {
        ("positive", "prepared.bin") => include_bytes!("prepared/positive/prepared.bin"),
        ("positive", "response.wire") => include_bytes!("prepared/positive/response.wire"),
        ("positive", "continuation.bin") => include_bytes!("prepared/positive/continuation.bin"),
        ("negative", "prepared.bin") => include_bytes!("prepared/negative/prepared.bin"),
        ("negative", "response.wire") => include_bytes!("prepared/negative/response.wire"),
        ("negative", "continuation.bin") => include_bytes!("prepared/negative/continuation.bin"),
        ("followup-positive", "prepared.bin") => include_bytes!("prepared/followup-positive/prepared.bin"),
        ("followup-positive", "response.wire") => include_bytes!("prepared/followup-positive/response.wire"),
        ("followup-positive", "continuation.bin") => include_bytes!("prepared/followup-positive/continuation.bin"),
        ("followup-negative", "prepared.bin") => include_bytes!("prepared/followup-negative/prepared.bin"),
        ("followup-negative", "response.wire") => include_bytes!("prepared/followup-negative/response.wire"),
        ("followup-negative", "continuation.bin") => include_bytes!("prepared/followup-negative/continuation.bin"),
        ("stale-positive", "prepared.bin") => include_bytes!("prepared/stale-positive/prepared.bin"),
        ("stale-positive", "response.wire") => include_bytes!("prepared/stale-positive/response.wire"),
        ("stale-positive", "continuation.bin") => include_bytes!("prepared/stale-positive/continuation.bin"),
        _ => panic!("unknown synthetic test vector"),
    }
}
