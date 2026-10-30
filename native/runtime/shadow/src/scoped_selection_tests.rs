//! R5: storage fixtures test retrieval, not native epistemic admission.
use super::*;
use shadow_contracts::{KnowledgeSelectionRequestWire as Request, SelectionRequester,
    SelectionPurpose, PROTOCOL_VERSION, SELECTION_MAX_CANDIDATES as LIMIT};

fn request(xi: &[&str], yi: &[&str]) -> Request {
    Request {version:PROTOCOL_VERSION, requester:SelectionRequester::AshtQuantum,
        purpose:SelectionPurpose::EvidenceCandidates,session_id:"retrieval-test".into(),
        parent_i0:"11".repeat(32),primitive_anchor:7,trace_id:"retrieval-test".into(),
        split_sha256:"22".repeat(32),xi:xi.iter().map(|s|s.to_string()).collect(),
        yi:yi.iter().map(|s|s.to_string()).collect(),upstream_hold_sha256:Some("33".repeat(32)),
        evidence_sha256:"44".repeat(32),max_positive:LIMIT,max_negative:LIMIT}
}
fn positive(v:&KnowledgeVault,id:u64,bytes:&[u8]) {
    v.write_primitive(id,0.9,0.8,0.7,500,ShadowPath::Fast,"science".into(),bytes.to_vec(),id+1).unwrap();
}
fn negative(v:&KnowledgeVault,id:u64,bytes:&[u8],reason:u32) {
    v.write_negative(id,bytes.to_vec(),NegativeContext{failure_reason_code:reason,
        process:"science".into(),mass_at_fail:0.25,frequency:1},id+1).unwrap();
}
#[test]
fn exact_pair_after_unrelated_capacity_is_retrieved_without_mutation() {
    let v=KnowledgeVault::new();
    for i in 0..u64::from(LIMIT)+5 {
        let bytes=format!("unrelated-{i}");
        positive(&v,i+1,bytes.as_bytes()); negative(&v,i+1000,bytes.as_bytes(),1);
    }
    positive(&v,5000,b"alpha"); negative(&v,6000,b"beta",9);
    let before=v.primitive.read().unwrap().iter().map(|e|e.access_count).collect::<Vec<_>>();
    let q=request(&["alpha"],&["beta"]);
    let (p,n)=v.select_bounded_candidates(&q).unwrap();
    assert_eq!(p.iter().map(|c|c.candidate_id).collect::<Vec<_>>(),[5000]);
    assert_eq!(n.iter().map(|c|c.candidate_id).collect::<Vec<_>>(),[6000]);
    assert_eq!(p[0].term_sha256,shadow_contracts::sha256_hex(&[b"ESSMAI/SELECTION/TERM/PRIMITIVE",b"alpha"]));
    assert_eq!(n[0].source_sha256,shadow_contracts::sha256_hex(&[
        b"ESSMAI/SELECTION/SOURCE/NEGATIVE",&6000u64.to_le_bytes(),&9u32.to_le_bytes(),b"beta"]));
    assert_eq!(n[0].confidence_fixed,7500);
    assert_eq!((n[0].security_y,n[0].trust_x,n[0].frequency),(0,0,1));
    assert_eq!(before,v.primitive.read().unwrap().iter().map(|e|e.access_count).collect::<Vec<_>>());
    let mut besa=q.clone();besa.requester=SelectionRequester::BesaNlight;
    besa.purpose=SelectionPurpose::XiYiStatistics;besa.upstream_hold_sha256=None;
    assert_eq!(v.select_bounded_candidates(&besa).unwrap(),(p,n));
}
#[test]
fn whole_proposition_is_not_tokenized_or_relabelled_as_many_facts() {
    let v=KnowledgeVault::new();
    positive(&v,1,b"Alpha beta"); negative(&v,2,b"Beta",7);
    assert_eq!(v.select_bounded_candidates(&request(&["Alpha"],&["beta"])).unwrap(),(vec![],vec![]));
    let (p,n)=v.select_bounded_candidates(&request(&["Alpha beta"],&["Beta"])).unwrap();
    assert_eq!(p.len(),1);assert_eq!(n.len(),1);
    let (p,n)=v.select_bounded_candidates(&request(&["Beta"],&["Alpha beta"])).unwrap();
    assert_eq!(p[0].polarity,1);assert_eq!(n[0].polarity,0);
    // Xi/Yi are axes; they do not rewrite the stored positive/negative verdict.
}
#[test]
fn saturated_relevant_negative_set_still_reports_the_full_bound() {
    let v=KnowledgeVault::new();
    positive(&v,1,b"alpha");
    for i in 0..u32::from(LIMIT)+1 {negative(&v,u64::from(i)+10,b"beta",i);}
    let (p,n)=v.select_bounded_candidates(&request(&["alpha"],&["beta"])).unwrap();
    assert_eq!(p.len(),1);assert_eq!(n.len(),usize::from(LIMIT));
    // Unchanged ASHT completeness guard rejects len == LIMIT; never quietly
    // reduce it to LIMIT-1 and claim the enumeration was complete.
    assert!(n.len()>=usize::from(LIMIT));
    let mut invalid=request(&["alpha"],&["beta"]);invalid.max_negative=0;
    assert!(v.select_bounded_candidates(&invalid).is_err());
}
#[test]
fn reopened_disk_keeps_exact_selection_and_does_not_append_on_read() {
    let path=std::env::temp_dir().join(format!("r5-scoped-selection-{}-{}.svlt",
        std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let q=request(&["alpha"],&["beta"]);
    let expected;
    {
        let v=KnowledgeVault::open_disk(&path).unwrap();
        positive(&v,11,b"alpha");negative(&v,12,b"beta",17);
        expected=v.select_bounded_candidates(&q).unwrap();
    }
    let before=std::fs::read(&path).unwrap();
    {
        let v=KnowledgeVault::open_disk(&path).unwrap();
        assert_eq!(v.select_bounded_candidates(&q).unwrap(),expected);
        assert_eq!(v.select_bounded_candidates(&q).unwrap(),expected);
    }
    assert_eq!(std::fs::read(&path).unwrap(),before);
    std::fs::remove_file(path).unwrap();
}
