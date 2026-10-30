//! PDS1 joins an explicit offline science query to an existing
//! PDF1/PDS1 question. Data only: no new map, authority, provider or warehouse.
use super::failure::{self, FailureDialogueInput};
use super::*;

pub const MAX_SCIENCE_QUERY_BYTES: usize = 16 * 1024;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedScienceInput {
    pub dialogue: FailureDialogueInput,
    pub parent_question_sha256: Hash,
    pub science_query: Vec<u8>,
    /// Explicit user-proposed relation, not verified relevance or admission.
    pub relevance_statement: String,
}
impl LinkedScienceInput {
    pub fn validate(&self) -> Result<(), String> {
        self.dialogue.validate()?;
        if self.dialogue.in_reply_to == [0; 32] || self.parent_question_sha256 == [0; 32] {
            return Err("PD_SCIENCE_PARENT_REQUIRED".into());
        }
        if self.science_query.is_empty() || self.science_query.len() > MAX_SCIENCE_QUERY_BYTES {
            return Err("PD_SCIENCE_QUERY_LIMIT".into());
        }
        text_valid(&self.relevance_statement, MAX_TEXT_BYTES)?;
        let actual = [
            &self.dialogue.positive,
            &self.dialogue.negative,
            &self.dialogue.distinction,
            &self.dialogue.source_reference,
        ]
        .map(String::as_str)
        .join("\n");
        if self.dialogue.user_text != actual {
            return Err("PD_SCIENCE_USER_FIELDS_BINDING".into());
        }
        Ok(())
    }
}
pub fn encode_linked_input(input: &LinkedScienceInput) -> Result<Vec<u8>, String> {
    input.validate()?;
    let mut out = b"PDS1".to_vec();
    put_bytes(
        &mut out,
        &failure::encode_failure_input(&input.dialogue)?,
        MAX_SOURCE_BYTES,
    )?;
    out.extend_from_slice(&input.parent_question_sha256);
    put_bytes(&mut out, &input.science_query, MAX_SCIENCE_QUERY_BYTES)?;
    put_text(&mut out, &input.relevance_statement)?;
    if out.len() > MAX_SOURCE_BYTES {
        return Err("PD_SCIENCE_SOURCE_LIMIT".into());
    }
    Ok(out)
}
pub fn decode_linked_input(bytes: &[u8]) -> Result<LinkedScienceInput, String> {
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err("PD_SCIENCE_SOURCE_LIMIT".into());
    }
    let mut reader = Reader { bytes, pos: 0 };
    if reader.take(4)? != b"PDS1" {
        return Err("PD_SCIENCE_VERSION".into());
    }
    let input = LinkedScienceInput {
        dialogue: failure::decode_failure_input(&reader.bytes(MAX_SOURCE_BYTES)?)?,
        parent_question_sha256: reader.hash()?,
        science_query: reader.bytes(MAX_SCIENCE_QUERY_BYTES)?,
        relevance_statement: reader.text()?,
    };
    reader.end()?;
    input.validate()?;
    if encode_linked_input(&input)? != bytes {
        return Err("PD_SCIENCE_NONCANONICAL".into());
    }
    Ok(input)
}
/// Retains the exact original selection/refusal and user fields for either kind.
pub fn decode_dialogue_source(bytes: &[u8]) -> Result<FailureDialogueInput, String> {
    match bytes.get(..4) {
        Some(b"PDF1") => failure::decode_failure_input(bytes),
        Some(b"PDS1") => Ok(decode_linked_input(bytes)?.dialogue),
        _ => Err("PD_DIALOGUE_SOURCE_VERSION".into()),
    }
}
/// Actual parent data are supplied by the Vault owner; hashes alone do not
/// authenticate a parent. Used again on the producer/consumer boundaries.
pub fn validate_linked_parent(
    input: &LinkedScienceInput,
    parent: &PdOperationRecord,
) -> Result<(), String> {
    input.validate()?;
    parent.validate()?;
    let original = decode_dialogue_source(&parent.request.source_input)?;
    let question_hash: Hash = Sha256::digest(parent.request.scoped_question.as_bytes()).into();
    if input.dialogue.in_reply_to != parent.record_id
        || input.parent_question_sha256 != question_hash
        || input.dialogue.identity != parent.request.identity
        || input.dialogue.selection_request != original.selection_request
        || input.dialogue.selection_response != original.selection_response
        || input.dialogue.refusal != original.refusal
    {
        return Err("PD_SCIENCE_PARENT_BINDING".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn truncated_unknown_and_oversized_frames_fail_boundedly() {
        for bytes in [
            b"".as_slice(),
            b"PDS1".as_slice(),
            b"PDS1\xff\xff\xff\xff".as_slice(),
            b"PDC1".as_slice(),
        ] {
            assert!(decode_linked_input(bytes).is_err());
        }
        assert!(decode_linked_input(&vec![0; MAX_SOURCE_BYTES + 1]).is_err());
        assert!(decode_dialogue_source(b"unknown").is_err());
    }
}

/// Versioned extraction only; never upgrades PDF1 into a science source.
pub fn science_query_from_source(bytes: &[u8]) -> Result<Vec<u8>, String> {
    match bytes.get(..4) {
        Some(b"PDS1") => Ok(decode_linked_input(bytes)?.science_query),
        Some(b"PDS2") | Some(b"PDS3") | Some(b"PDS4") | Some(b"PDS5") => Ok(super::iz_science::decode_input(bytes)?.science_query),
        _ => Err("PD_SCIENCE_SOURCE_VERSION".into()),
    }
}
pub fn validate_science_record(record: &PdOperationRecord) -> Result<(), String> {
    record.validate()?;
    match record.request.source_input.get(..4) {
        Some(b"PDS1") => {
            failure::validate_failure_operation(&record.request)?;
        }
        Some(b"PDS2") | Some(b"PDS3") | Some(b"PDS4") | Some(b"PDS5") => {
            super::iz_science::validate_operation(&record.request)?;
        }
        _ => return Err("PD_SCIENCE_SOURCE_VERSION".into()),
    }
    Ok(())
}
pub fn science_checkpoint_raw(record: &PdOperationRecord) -> Result<&[u8], String> {
    validate_science_record(record)?;
    if super::iz_science::source_is_science(&record.request.source_input) {
        return super::iz_science::science_checkpoint_raw(&record.request);
    }
    record
        .request
        .checkpoint
        .strip_prefix(failure::FAILURE_CHECKPOINT_PREFIX)
        .ok_or("PD_SCIENCE_CHECKPOINT_POLICY".into())
}
