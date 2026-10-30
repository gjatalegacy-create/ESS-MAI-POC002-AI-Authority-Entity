// R15: single source of the Xi/Yi split wire `xi:a,b|yi:c` shared by Light,
// Quantum and Shadow. Exact lines of quantum/src/lab_contracts/collapse.rs (the
// PrimitiveSplit block and its two tests), plus the delimiter rule below.

/// (Xi, Yi) — dy hapësirat primitive që lindin nga PA në Light.
///
/// Xi = hapësira e objektit HYRËS (kandidatët/kushtet e asaj që përshkruhet).
/// Yi = hapësira e objektit DALËS (boshtet e përgjigjes që kërkohet).
///
/// INVARIANTET (të detyruara nga ndërtuesi në Light):
///   • Xi ∩ Yi = ∅  (asnjë term në të dy hapësirat)
///   • Xi ∪ Yi = termat primitivë të PA-së (asgjë s'humbet, asgjë s'shpiket)
#[derive(Debug, Clone, PartialEq)]
pub struct PrimitiveSplit {
    pub xi: Vec<String>,
    pub yi: Vec<String>,
}

impl PrimitiveSplit {
    /// A ka material të mjaftueshëm për kolaps? (të dyja hapësirat jo-bosh
    /// nuk kërkohen — një pyetje mund të ketë vetëm Xi; POR të dyja bosh = jo)
    pub fn has_material(&self) -> bool {
        !(self.xi.is_empty() && self.yi.is_empty())
    }

    /// Pure compute codec: safe legacy bytes or exact versioned fields.
    /// Encoding carries data only; existing LGC and authority checks still apply.
    pub fn to_wire(&self) -> String {
        crate::collapse_wire::encode_split(&self.xi, &self.yi)
    }

    pub fn from_wire(xi_seg: &str, yi_seg: &str) -> Option<Self> {
        let (xi, yi) = crate::collapse_wire::decode_split(xi_seg, yi_seg)?;
        Some(Self { xi, yi })
    }
}

impl PrimitiveSplit {
    /// R15: true when every term crosses `to_wire` -> `from_wire` byte for byte.
    /// A bound term digest must never be silently re-normalised by the wire.
    pub fn is_wire_exact(&self) -> bool {
        let wire = self.to_wire();
        match wire.split_once('|') {
            Some((xi, yi)) => Self::from_wire(xi, yi).as_ref() == Some(self),
            None => false,
        }
    }
}

/// Transport representability, not semantic admission or material sufficiency.
/// Versioned framing also preserves delimiters and empty fields exactly.
pub fn is_wire_term(term: &str) -> bool {
    PrimitiveSplit { xi: vec![term.to_owned()], yi: Vec::new() }.is_wire_exact()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r16_actual_wire_preserves_bound_comma_and_pipe() {
        let split = PrimitiveSplit {
            xi: vec!["jd_part2=54388.0,".into()], yi: vec!["provë|burim~ë".into()],
        };
        let wire = split.to_wire();
        let (xi, yi) = wire.split_once('|').unwrap();
        assert_eq!(PrimitiveSplit::from_wire(xi, yi), Some(split));
        assert_eq!(wire.matches('|').count(), 1);
    }

    #[test]
    fn split_wire_roundtrip() {
        let s = PrimitiveSplit {
            xi: vec!["flamur".into(), "kuq".into(), "zi".into()],
            yi: vec!["shqiponjë".into()],
        };
        let wire = s.to_wire();
        assert_eq!(wire, "xi:flamur,kuq,zi|yi:shqiponjë");
        let segs: Vec<&str> = wire.split('|').collect();
        let back = PrimitiveSplit::from_wire(segs[0], segs[1]).expect("parse");
        assert_eq!(back, s);
    }

    #[test]
    fn split_empty_yi_allowed_but_both_empty_no_material() {
        let only_xi = PrimitiveSplit {
            xi: vec!["kimi".into()],
            yi: vec![],
        };
        assert!(only_xi.has_material());
        let none = PrimitiveSplit {
            xi: vec![],
            yi: vec![],
        };
        assert!(!none.has_material());
        // wire i zbrazët parsohet i sigurt
        let p = PrimitiveSplit::from_wire("xi:", "yi:").expect("parse bosh");
        assert!(!p.has_material());
    }

    #[test]
    fn r16_delimiters_are_preserved_by_versioned_wire() {
        let exact = PrimitiveSplit { xi: vec!["era00".into(), "jd_part2=54388.0".into()], yi: vec![] };
        assert!(exact.is_wire_exact());
        for bad in ["jd_part2=54388.0,", "a|b", " x", "", "a\nb"] {
            assert!(PrimitiveSplit { xi: vec![bad.into()], yi: vec![] }.is_wire_exact(), "{bad:?}");
        }
    }
}
