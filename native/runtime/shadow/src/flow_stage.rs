//! Typestate contract for the existing Shadow verification diary.
//!
//! The types in this module do not create a second runtime machine. They make
//! the already-authoritative VDS transition order visible to the compiler.

use crate::types::ShadowError;
use std::fmt;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageTag {
    AwaitingAnchor = 0,
    AnchorTrusted = 1,
    ProposalReceived = 2,
    ProcessJudged = 3,
    StructureHeld = 4,
    DiaryVerdict = 5,
}

mod sealed {
    pub trait Sealed {}
}

pub trait FlowStage: sealed::Sealed {
    const TAG: StageTag;
}

macro_rules! define_stage {
    ($name:ident, $tag:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name;

        impl sealed::Sealed for $name {}

        impl FlowStage for $name {
            const TAG: StageTag = StageTag::$tag;
        }
    };
}

define_stage!(AwaitingAnchor, AwaitingAnchor);
define_stage!(AnchorTrusted, AnchorTrusted);
define_stage!(ProposalReceived, ProposalReceived);
define_stage!(ProcessJudged, ProcessJudged);
define_stage!(StructureHeld, StructureHeld);
define_stage!(DiaryVerdict, DiaryVerdict);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowBreach {
    WrongPhase(&'static str),
    AnchorUntrusted(&'static str),
    ProposalInvalid(&'static str),
    ProcessContradiction(&'static str),
    StructureBroken(&'static str),
    ThroneRefusal(&'static str),
}

impl FlowBreach {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::WrongPhase(_) => "WRONG_PHASE",
            Self::AnchorUntrusted(_) => "ANCHOR_UNTRUSTED",
            Self::ProposalInvalid(_) => "PROPOSAL_INVALID",
            Self::ProcessContradiction(_) => "PROCESS_CONTRADICTION",
            Self::StructureBroken(_) => "STRUCTURE_BROKEN",
            Self::ThroneRefusal(_) => "THRONE_REFUSAL",
        }
    }

    pub const fn detail(&self) -> &'static str {
        match self {
            Self::WrongPhase(text)
            | Self::AnchorUntrusted(text)
            | Self::ProposalInvalid(text)
            | Self::ProcessContradiction(text)
            | Self::StructureBroken(text)
            | Self::ThroneRefusal(text) => text,
        }
    }
}

impl fmt::Display for FlowBreach {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code(), self.detail())
    }
}

impl std::error::Error for FlowBreach {}

impl From<FlowBreach> for ShadowError {
    fn from(breach: FlowBreach) -> Self {
        ShadowError::SealInvalid(format!("ShadowFlow {}", breach))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_tags_are_stable_and_total() {
        let tags = [
            AwaitingAnchor::TAG,
            AnchorTrusted::TAG,
            ProposalReceived::TAG,
            ProcessJudged::TAG,
            StructureHeld::TAG,
            DiaryVerdict::TAG,
        ];
        assert_eq!(tags.len(), 6);
        assert_eq!(tags.map(|tag| tag as u8), [0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn every_breach_has_a_named_code() {
        let breaches = [
            FlowBreach::WrongPhase("x"),
            FlowBreach::AnchorUntrusted("x"),
            FlowBreach::ProposalInvalid("x"),
            FlowBreach::ProcessContradiction("x"),
            FlowBreach::StructureBroken("x"),
            FlowBreach::ThroneRefusal("x"),
        ];
        assert!(breaches.iter().all(|breach| !breach.code().is_empty()));
    }
}
