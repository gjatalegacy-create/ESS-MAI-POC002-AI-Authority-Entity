// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  LAYER3/SHADOW_NIGHT_WATCH.RS — Sentineli i Jashtëm (Shadow)         ║
// ║  GJATA LEGACY™ — AIGovern-Cyber Shadow                          (LAYER 3)    ║
// ║                                                                          ║
// ║  shadow_night_watch është një GJYKATËS tjetër — por gjykon duke marrë   ║
// ║  përditësime për policy + info nga web NËPËRMJET device (kurrë duke      ║
// ║  prekur web-in drejtpërdrejt). Verifikon me 7 shtresa info të gjera dhe  ║
// ║  të plota, dhe gjykon drejt 0/1. I lidhur me Shtresën 3 lab-to-lab.    ║
// ║                                                                          ║
// ║  KUFIRI KRITIK (nga orientimi):                                       ║
// ║   • Night Watch NUK hyn te shadow_judiciary — është sentinel i jashtëm. ║
// ║   • Vjen PARA gjykimit final, mbledh sinjale, gjykon 0/1 mbi to.        ║
// ║   • Nuk e zëvendëson verdiktin suprem — është një shtresë verifikimi    ║
// ║     shtesë me info të gjerë. Zero if/else.                            ║
// ╚══════════════════════════════════════════════════════════════════════════╝

use crate::lab_contracts_v11::watch::{NightWatchReport, SevenLayerInfo};

// ─────────────────────────────────────────────────────────────────────────────
// KONSTANTET — pragjet e gjykimit të sentinelit
// ─────────────────────────────────────────────────────────────────────────────

/// Pragu i agregatit të 7 shtresave për 0/1 = 1.
pub const NIGHT_WATCH_PASS: f32 = 0.65;
/// Numri minimal i burimeve për gjykim të vlefshëm.
pub const NIGHT_WATCH_MIN_SOURCES: u32 = 3;

// ─────────────────────────────────────────────────────────────────────────────
// NIGHT WATCH VERDICT — gjykimi i sentinelit
// ─────────────────────────────────────────────────────────────────────────────

/// Arsyeja e gjykimit të sentinelit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchRejection {
    None,             // kaloi
    Incomplete,       // raporti i paplotë (7 shtresa/burime)
    LowConfidence,    // agregati nën prag
    NotViaDevice,     // s'u mblodh nëpërmjet device (shkelje parimi)
    OutOfDomain,      // R15: një shtresë jashtë [0,1] ose jo-numerike
    ClaimMismatch,    // R15: raporti i përket një pretendimi tjetër
}

impl WatchRejection {
    pub fn label(self) -> &'static str {
        match self {
            WatchRejection::None          => "WATCH_NONE",
            WatchRejection::Incomplete    => "INCOMPLETE",
            WatchRejection::LowConfidence => "LOW_CONFIDENCE",
            WatchRejection::NotViaDevice  => "NOT_VIA_DEVICE",
            WatchRejection::OutOfDomain   => "OUT_OF_DOMAIN",
            WatchRejection::ClaimMismatch => "CLAIM_MISMATCH",
        }
    }
}

/// Verdikti i sentinelit — 0/1 mbi 7 shtresat e info-s.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WatchVerdict {
    /// 0/1 — a kaloi verifikimin e gjerë.
    pub bit:        u8,
    /// Besueshmëria e agreguar [0,1].
    pub confidence: f32,
    /// Arsyeja nëse s'kaloi.
    pub rejection:  WatchRejection,
}

// ─────────────────────────────────────────────────────────────────────────────
// NIGHT WATCH — sentineli që gjykon mbi 7 shtresat
// ─────────────────────────────────────────────────────────────────────────────

pub struct NightWatch;

impl NightWatch {
    /// judge — gjykon 0/1 mbi raportin e 7 shtresave.
    ///
    /// Prioritet refuzimi (match): via device > plotësi > besueshmëri.
    /// Zero if — match shterues.
    pub fn judge(report: &NightWatchReport) -> WatchVerdict {
        let via_device = report.via_device;
        let in_domain = Self::layers_in_domain(&report.layers);
        let complete = report.is_complete();
        let enough_sources = report.sources_count >= NIGHT_WATCH_MIN_SOURCES;
        let confidence = match in_domain {
            true => report.confidence(),
            false => 0.0,
        };
        let conf_ok = confidence >= NIGHT_WATCH_PASS;

        // Vendimi + arsyeja (prioritet: device > domeni > plotësi+burime > besueshmëri).
        let (bit, rejection) = match (via_device, in_domain, complete & enough_sources, conf_ok) {
            (false, _, _, _)    => (0u8, WatchRejection::NotViaDevice),
            (true, false, _, _) => (0u8, WatchRejection::OutOfDomain),
            (true, true, false, _) => (0u8, WatchRejection::Incomplete),
            (true, true, true, false) => (0u8, WatchRejection::LowConfidence),
            (true, true, true, true)  => (1u8, WatchRejection::None),
        };

        WatchVerdict {
            bit,
            confidence,
            rejection,
        }
    }

    /// R15 (F1 verification (3)): a report judges only its own claim.
    pub fn judge_for(report: &NightWatchReport, claim_id: u64) -> WatchVerdict {
        match report.claim_id == claim_id {
            true => Self::judge(report),
            false => WatchVerdict { bit: 0, confidence: 0.0, rejection: WatchRejection::ClaimMismatch },
        }
    }

    /// Every one of the seven layers is a finite measure in [0,1].
    fn layers_in_domain(l: &SevenLayerInfo) -> bool {
        [
            l.policy_alignment, l.literature_support, l.source_consensus, l.recency,
            l.credibility, l.contradiction_free, l.cross_coverage,
        ]
        .iter()
        .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
    }

    /// gather_summary — përmbledhje e shkurtër e sinjaleve (për report). Zero if.
    pub fn gather_summary(report: &NightWatchReport) -> String {
        format!(
            "claim={} | confidence={:.2} | sources={} | via_device={}",
            report.claim_id,
            report.confidence(),
            report.sources_count,
            report.via_device
        )
    }

    /// strongest_layer — cila nga 7 shtresat është më e fortë. Zero if — fold.
    pub fn strongest_layer(info: &SevenLayerInfo) -> &'static str {
        // Tabela e shtresave me vlerat.
        let layers = [
            ("policy", info.policy_alignment),
            ("literature", info.literature_support),
            ("consensus", info.source_consensus),
            ("recency", info.recency),
            ("credibility", info.credibility),
            ("contradiction_free", info.contradiction_free),
            ("cross_coverage", info.cross_coverage),
        ];
        // Gjej max via fold (zero if).
        let best = layers.iter().fold(("none", -1.0f32), |acc, &(name, val)| {
            let take = (val > acc.1) as u32;
            // take=1 → (name,val); take=0 → acc. Zero if — match.
            match take {
                1 => (name, val),
                _ => acc,
            }
        });
        best.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_report() -> NightWatchReport {
        NightWatchReport {
            claim_id: 1,
            layers: SevenLayerInfo {
                policy_alignment: 0.8, literature_support: 0.7, source_consensus: 0.9,
                recency: 0.6, credibility: 0.85, contradiction_free: 0.75, cross_coverage: 0.7,
            },
            via_device: true, sources_count: 5, gathered_at: 100,
        }
    }

    #[test]
    fn passes_strong_report() {
        let v = NightWatch::judge(&full_report());
        assert_eq!(v.bit, 1);
        assert_eq!(v.rejection, WatchRejection::None);
        assert!(v.confidence >= NIGHT_WATCH_PASS);
    }

    #[test]
    fn rejects_not_via_device() {
        let mut r = full_report();
        r.via_device = false;
        let v = NightWatch::judge(&r);
        assert_eq!(v.bit, 0);
        assert_eq!(v.rejection, WatchRejection::NotViaDevice);
    }

    #[test]
    fn rejects_incomplete() {
        let mut r = full_report();
        r.sources_count = 1; // nën minimumin
        let v = NightWatch::judge(&r);
        assert_eq!(v.bit, 0);
        assert_eq!(v.rejection, WatchRejection::Incomplete);
    }

    #[test]
    fn rejects_low_confidence() {
        let mut r = full_report();
        // Të gjitha shtresat të ulëta por jo-zero (plot por dobët).
        r.layers = SevenLayerInfo {
            policy_alignment: 0.3, literature_support: 0.2, source_consensus: 0.3,
            recency: 0.2, credibility: 0.3, contradiction_free: 0.2, cross_coverage: 0.3,
        };
        let v = NightWatch::judge(&r);
        assert_eq!(v.bit, 0);
        assert_eq!(v.rejection, WatchRejection::LowConfidence);
    }

    #[test]
    fn strongest_layer_found() {
        let info = SevenLayerInfo {
            policy_alignment: 0.8, literature_support: 0.7, source_consensus: 0.95,
            recency: 0.6, credibility: 0.85, contradiction_free: 0.75, cross_coverage: 0.7,
        };
        assert_eq!(NightWatch::strongest_layer(&info), "consensus"); // 0.95
    }

    #[test]
    fn pending_report_rejected() {
        let r = NightWatchReport::pending(1);
        let v = NightWatch::judge(&r);
        assert_eq!(v.bit, 0); // pa shtresa/burime
    }

    /// R15 (F1 verification (3)): a layer outside [0,1] or non-finite gives bit 0.
    #[test]
    fn r15_out_of_domain_layer_is_rejected() {
        for bad in [f32::INFINITY, f32::NAN, 1.5, -0.1] {
            let mut r = full_report();
            r.layers.source_consensus = bad;
            assert_eq!(NightWatch::judge(&r).bit, 0, "{bad}");
            assert_eq!(NightWatch::judge(&r).rejection, WatchRejection::OutOfDomain);
        }
        let r = full_report();
        assert_eq!(NightWatch::judge_for(&r, r.claim_id).bit, 1);
        assert_eq!(NightWatch::judge_for(&r, r.claim_id + 1).rejection, WatchRejection::ClaimMismatch);
    }
}
