//! Derived content addresses over the existing ordered storage, never an authority.
//! The owning RwLock covers rows and index together. No row handle escapes it.
use super::{candidate_content_sha256, LegacyEntry, NegativeEntry, PrimitiveEntry};
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use std::cmp::Reverse;
use std::ops::{Deref, DerefMut};
use std::sync::OnceLock;

pub(super) trait Content { fn bytes(&self) -> &[u8]; }
impl Content for PrimitiveEntry { fn bytes(&self) -> &[u8] { &self.raw_bytes } }
impl Content for NegativeEntry { fn bytes(&self) -> &[u8] { &self.raw_bytes } }
impl Content for LegacyEntry { fn bytes(&self) -> &[u8] { &self.raw_bytes } }

#[derive(Debug)]
pub(super) struct CandidateStore<T> {
    rows: Vec<T>,
    by_content: OnceLock<BTreeMap<[u8; 32], Vec<usize>>>,
}
impl<T: Content> CandidateStore<T> {
    pub(super) fn new() -> Self { Self { rows: Vec::new(), by_content: OnceLock::new() } }
    #[cfg(test)]
    pub(super) fn r42_index_is_warm(&self) -> bool { self.by_content.get().is_some() }
    pub(super) fn push(&mut self, value: T) {
        if let Some(index) = self.by_content.get_mut() {
            index.entry(candidate_content_sha256(value.bytes())).or_default().push(self.rows.len());
        }
        self.rows.push(value);
    }
    // A new row published after WAL append must not allocate a warmed index.
    pub(super) fn push_unindexed(&mut self, value: T) {
        self.by_content.take();
        self.rows.push(value);
    }
    pub(super) fn update_at<R>(&mut self, row: usize, update: impl FnOnce(&mut T) -> R) -> Option<R> {
        let entry = self.rows.get(row)?;
        let prior_hash = self.by_content.get().map(|_| candidate_content_sha256(entry.bytes()));
        // Remove the cache before exposing mutation, so unwinding leaves it cold.
        let index = self.by_content.take();
        let result = update(&mut self.rows[row]);
        if let Some(index) = index {
            if Some(candidate_content_sha256(self.rows[row].bytes())) == prior_hash {
                // Row position and bucket key are unchanged. Exact bytes are still
                // checked by matching; this does not establish candidate identity.
                self.by_content.set(index).expect("exclusive update keeps index absent");
            }
        }
        Some(result)
    }
    pub(super) fn try_reserve_for_mutation(&mut self, additional: usize)
        -> Result<(), std::collections::TryReserveError> {
        // Reserve rows before durable append; subsequent pushes must not allocate
        // an already-warmed derived index while publishing the committed change.
        self.by_content.take();
        self.rows.try_reserve(additional)
    }
    pub(super) fn remove(&mut self, row: usize) -> T {
        self.by_content.take(); // Vec::remove shifts all following positions.
        self.rows.remove(row)
    }
    // I(c) = SHA256(domain || len(c) || c). B(h) contains increasing row positions.
    // Merge only requested buckets in row order, then require exact bytes.
    // A collision may add a comparison; it cannot manufacture a matching object.
    pub(super) fn matching<'a>(&'a self, terms: &'a [&'a [u8]]) -> impl Iterator<Item=&'a T> + 'a {
        // Initialization is deferred until next(): take(0) and empty requests
        // must not scan an unrelated store or allocate its index.
        let mut search = None;
        std::iter::from_fn(move || {
            if terms.is_empty() { return None; }
            let (buckets, heap) = search.get_or_insert_with(|| {
                let index = self.by_content.get_or_init(|| {
                    let mut result: BTreeMap<[u8;32],Vec<usize>> = BTreeMap::new();
                    for (row,value) in self.rows.iter().enumerate() {
                        result.entry(candidate_content_sha256(value.bytes())).or_default().push(row);
                    }
                    result
                });
                let keys: BTreeSet<_> = terms.iter().map(|term|candidate_content_sha256(term)).collect();
                let buckets: Vec<_> = keys.iter().filter_map(|key|index.get(key)).collect();
                let heap: BinaryHeap<_> = buckets.iter().enumerate()
                    .filter_map(|(bucket,rows)|rows.first().map(|row|Reverse((*row,bucket,0usize)))).collect();
                (buckets,heap)
            });
            while let Some(Reverse((row,bucket,offset)))=heap.pop() {
                if let Some(next)=buckets[bucket].get(offset+1) { heap.push(Reverse((*next,bucket,offset+1))); }
                let value=&self.rows[row];
                if terms.iter().any(|term|*term==value.bytes()) { return Some(value); }
            }
            None
        })
    }
}
impl<T> Deref for CandidateStore<T> {
    type Target=[T];
    fn deref(&self)->&[T] { &self.rows }
}
impl<T> DerefMut for CandidateStore<T> {
    fn deref_mut(&mut self)->&mut [T] {
        self.by_content.take(); // Any mutable borrow may change bytes or their order.
        &mut self.rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Debug)] struct Row(Vec<u8>);
    impl Content for Row {fn bytes(&self)->&[u8] {&self.0}}
    fn selected(store:&CandidateStore<Row>,terms:&[&[u8]])->Vec<Vec<u8>> {
        store.matching(terms).map(|r|r.0.clone()).collect()
    }
    #[test]
    fn addressed_update_invalidates_changed_content_and_preserves_missing_row() {
        let mut store = CandidateStore::new();
        store.push(Row(b"alpha".to_vec())); store.push(Row(b"beta".to_vec()));
        assert_eq!(selected(&store, &[b"alpha"]), [b"alpha".to_vec()]);
        assert_eq!(store.update_at(9, |_| panic!("missing row must not invoke mutation")), None::<()>);
        assert!(store.by_content.get().is_some());
        store.update_at(0, |row| row.0 = b"gamma".to_vec()).unwrap();
        assert!(store.by_content.get().is_none());
        assert!(selected(&store, &[b"alpha"]).is_empty());
        assert_eq!(selected(&store, &[b"gamma", b"beta"]), [b"gamma".to_vec(), b"beta".to_vec()]);
    }

    #[test]
    fn addressed_update_panic_cannot_restore_stale_index() {
        let mut store = CandidateStore::new(); store.push(Row(b"alpha".to_vec()));
        assert_eq!(selected(&store, &[b"alpha"]), [b"alpha".to_vec()]);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = store.update_at(0, |row| { row.0 = b"beta".to_vec(); panic!("mutation interrupted") });
        }));
        assert!(result.is_err()); assert!(store.by_content.get().is_none());
        assert!(selected(&store, &[b"alpha"]).is_empty());
        assert_eq!(selected(&store, &[b"beta"]), [b"beta".to_vec()]);
    }

    #[test]
    fn unindexed_append_keeps_old_cold_publication_behavior() {
        let mut store = CandidateStore::new(); store.push(Row(b"alpha".to_vec()));
        assert_eq!(selected(&store, &[b"alpha"]), [b"alpha".to_vec()]);
        store.push_unindexed(Row(b"beta".to_vec()));
        assert!(store.by_content.get().is_none());
        assert_eq!(selected(&store, &[b"beta", b"alpha"]), [b"alpha".to_vec(), b"beta".to_vec()]);
    }

    #[test]
    fn addressed_metadata_update_visits_only_target_payload_without_copying_rows() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        // No Clone implementation: the update borrows the existing payload.
        struct Counted { raw: Vec<u8>, hits: u32, visits: AtomicUsize, bytes: AtomicUsize }
        impl Content for Counted {
            fn bytes(&self) -> &[u8] {
                self.visits.fetch_add(1, Ordering::Relaxed);
                self.bytes.fetch_add(self.raw.len(), Ordering::Relaxed); &self.raw
            }
        }
        fn populated() -> CandidateStore<Counted> {
            let mut store = CandidateStore::new();
            for n in 0..10_000 {
                let mut raw = format!("term-{n:05}:").into_bytes(); raw.resize(256, b'x');
                store.push(Counted { raw, hits: 0, visits: AtomicUsize::new(0), bytes: AtomicUsize::new(0) });
            }
            store
        }
        let mut baseline = populated(); let mut retained = populated();
        let requested = retained[9_999].raw.clone(); let terms = [requested.as_slice()];
        for store in [&baseline, &retained] {
            assert_eq!(store.matching(&terms).count(), 1);
            for row in store.iter() { row.visits.store(0, Ordering::Relaxed); row.bytes.store(0, Ordering::Relaxed); }
        }
        let pointer = retained[9_999].raw.as_ptr(); let capacity = retained[9_999].raw.capacity();
        baseline[9_999].hits += 1; // Existing unrestricted mutation invalidates all addresses.
        assert_eq!(retained.update_at(9_999, |row| { row.hits += 1; row.hits }), Some(1));
        assert!(retained.by_content.get().is_some());
        assert_eq!((retained[9_999].raw.as_ptr(), retained[9_999].raw.capacity()), (pointer, capacity));
        assert_eq!(baseline.matching(&terms).count(), 1); assert_eq!(retained.matching(&terms).count(), 1);
        let counts = |store: &CandidateStore<Counted>| store.iter().fold((0, 0), |(visits, bytes), row|
            (visits + row.visits.load(Ordering::Relaxed), bytes + row.bytes.load(Ordering::Relaxed)));
        assert_eq!(counts(&baseline), (10_001, 2_560_256));
        assert_eq!(counts(&retained), (3, 768));
        assert!(retained.iter().take(9_999).all(|row| row.visits.load(Ordering::Relaxed) == 0));
        assert_eq!(baseline[9_999].hits, retained[9_999].hits);
        println!("R42_INDEX_WORK {{\"rows\":10000,\"payload_bytes_per_row\":256,\"baseline_visits\":10001,\"baseline_bytes\":2560256,\"retained_visits\":3,\"retained_bytes\":768,\"scope\":\"software payload visits, not physical bandwidth\"}}");
    }

    #[test]
    fn r36_index_preserves_order_exact_bytes_duplicates_and_punctuation() {
        let mut store=CandidateStore::new();
        for raw in [b"alpha".as_slice(),b"a,b",b"beta",b"alpha",b"ab"] {store.push(Row(raw.to_vec()));}
        let terms=[b"beta".as_slice(),b"alpha",b"alpha",b"a,b"];
        assert_eq!(selected(&store,&terms),[b"alpha".to_vec(),b"a,b".to_vec(),b"beta".to_vec(),b"alpha".to_vec()]);
        assert_eq!(store.matching(&terms).take(2).map(|r|r.0.clone()).collect::<Vec<_>>(),[b"alpha".to_vec(),b"a,b".to_vec()]);
        assert!(selected(&store,&[b"Alpha"]).is_empty());assert!(selected(&store,&[]).is_empty());
    }
    #[test]
    fn r36_index_append_remove_mutation_and_reorder_do_not_leave_stale_addresses() {
        let mut store=CandidateStore::new();
        for raw in [b"a",b"b",b"c"] {store.push(Row(raw.to_vec()));}
        assert_eq!(selected(&store,&[b"c"]),[b"c".to_vec()]);
        store.push(Row(b"d".to_vec()));assert_eq!(selected(&store,&[b"d"]),[b"d".to_vec()]);
        store.remove(0);assert_eq!(selected(&store,&[b"c"]),[b"c".to_vec()]);
        store[0].0=b"x".to_vec();assert!(selected(&store,&[b"b"]).is_empty());
        assert_eq!(selected(&store,&[b"x"]),[b"x".to_vec()]);
        store.reverse();
        assert_eq!(selected(&store,&[b"x",b"c",b"d"]),[b"d".to_vec(),b"c".to_vec(),b"x".to_vec()]);
    }
    #[test]
    fn r36_hash_bucket_is_not_content_equality() {
        let mut store=CandidateStore::new();
        store.push(Row(b"alpha".to_vec()));store.push(Row(b"beta".to_vec()));
        assert_eq!(selected(&store,&[b"beta"]),[b"beta".to_vec()]);
        // Inject an auxiliary bucket collision, not a real SHA collision.
        store.by_content.get_mut().unwrap().insert(candidate_content_sha256(b"beta"),vec![0,1]);
        assert_eq!(selected(&store,&[b"beta"]),[b"beta".to_vec()]);
    }
    #[test]
    fn r36_index_matches_linear_reference_through_500_state_changes() {
        let mut store=CandidateStore::new();
        for step in 0..500usize {
            store.push(Row(format!("term-{}",step%17).into_bytes()));
            if step%7==0 && store.len()>2 {store.remove(1);}
            if step%13==0 {store[0].0=format!("edited-{step}").into_bytes();}
            let a=format!("term-{}",step%17);let b=format!("term-{}",(step+3)%17);
            let terms=[a.as_bytes(),b.as_bytes(),a.as_bytes()];
            let expected=store.iter().filter(|row|terms.contains(&row.bytes())).map(|r|r.0.clone()).collect::<Vec<_>>();
            assert_eq!(selected(&store,&terms),expected,"step {step}");
        }
    }
    #[test]
    fn r36_zero_capacity_or_empty_query_never_builds_an_index() {
        let mut store=CandidateStore::new();store.push(Row(b"unrelated".to_vec()));
        assert_eq!(store.matching(&[b"unrelated"]).take(0).count(),0);
        assert!(store.by_content.get().is_none());
        assert_eq!(store.matching(&[]).count(),0);
        assert!(store.by_content.get().is_none());
    }
    #[test]
    fn r36_warm_lookup_reads_only_addressed_content() {
        use std::sync::atomic::{AtomicUsize,Ordering};
        struct Counted {raw:Vec<u8>,reads:AtomicUsize}
        impl Content for Counted {fn bytes(&self)->&[u8] {self.reads.fetch_add(1,Ordering::Relaxed);&self.raw}}
        let mut store=CandidateStore::new();
        for n in 0..10000 {store.push(Counted{raw:format!("term-{n}").into_bytes(),reads:AtomicUsize::new(0)});}
        let terms=[b"term-9999".as_slice()];
        assert_eq!(store.matching(&terms).count(),1);
        for row in store.iter() {row.reads.store(0,Ordering::Relaxed);}
        assert_eq!(store.matching(&terms).count(),1);
        assert_eq!(store.iter().map(|r|r.reads.load(Ordering::Relaxed)).sum::<usize>(),1);
    }
}
