//! A concurrent Least Recently Used cache that holds a limited number of entries, according to the
//! size of each entry.  The implementation is a concurrent, but not lock-free, structure that
//! marries a map with a linked list.
//!
//! Values for the LeastRecentlyUsedCache must be clonable.  It's expected to put them within an
//! `Arc<V>` for fast cloning for types that require more than a simple set of allocations.  The
//! clone happens under a lock, so it's on the person caring about performance to consider the cost
//! of the clone.

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex, MutexGuard};

/////////////////////////////////////////////// Value //////////////////////////////////////////////

/// A Value for the LeastRecentlyUsed cache.  The value has an approximate_size method that is used
/// to compute the total size of the cache.
pub trait Value: Clone {
    /// The approximate size (in bytes) of this value.
    fn approximate_size(&self) -> usize;
}

impl Value for u64 {
    fn approximate_size(&self) -> usize {
        8
    }
}

impl Value for String {
    fn approximate_size(&self) -> usize {
        self.len()
    }
}

impl Value for Vec<u8> {
    fn approximate_size(&self) -> usize {
        self.len()
    }
}

impl<V: Value> Value for Arc<V> {
    fn approximate_size(&self) -> usize {
        <V as Value>::approximate_size(self)
    }
}

////////////////////////////////////// LeastRecentlyUsedCache //////////////////////////////////////

/// A least-recently-used (LRU) cache.  It's essentially a hash table and linked list that are kept
/// in sync with each other.  It is assumed that keys will be of constant size (e.g., a setsum and
/// offset for blocks within a file, or just the setsum for open file handles).  The size of the
/// key will be used, which doesn't cover allocations within.
pub struct LeastRecentlyUsedCache<K: Clone + Eq + Hash, V: Value> {
    capacity: usize,
    state: Mutex<State<K, V>>,
}

impl<K: Clone + Eq + Hash, V: Value> LeastRecentlyUsedCache<K, V> {
    /// Create a new LRU with the specified capacity in bytes.
    pub fn new(capacity: usize) -> Self {
        let state = Mutex::new(State {
            size: 0,
            head: std::ptr::null_mut(),
            tail: std::ptr::null_mut(),
            keys: HashMap::new(),
        });
        Self { capacity, state }
    }

    /// Insert (K, V) into the LRU, overwriting any existing key/value.
    pub fn insert(&self, key: K, value: V) {
        let ptr = Node::new(key.clone(), value);
        let mut state = self.state.lock().unwrap();
        state = self.insert_helper(state, ptr, key);
        while state.size > self.capacity && !state.head.is_null() {
            // SAFETY(rescrv):  We only held a reference to `node` and we dropped it before this.
            state = unsafe { self.remove_lru(state) };
        }
    }

    /// Insert (K, V) into the LRU, overwriting any existing key/value, and not evicting any
    /// entries.
    pub fn insert_no_evict(&self, key: K, value: V) {
        let ptr = Node::new(key.clone(), value);
        let state = self.state.lock().unwrap();
        drop(self.insert_helper(state, ptr, key));
    }

    /// Return the size of the LRU.
    pub fn approximate_size(&self) -> usize {
        // SAFETY(rescrv):  Mutex poisoning.
        self.state.lock().unwrap().size
    }

    /// Insert the node into the LRU, overwriting any existing key/value, but not evicting
    /// anything.
    fn insert_helper<'a>(
        &self,
        mut state: MutexGuard<'a, State<K, V>>,
        ptr: *mut Node<K, V>,
        key: K,
    ) -> MutexGuard<'a, State<K, V>> {
        // SAFETY(rescrv):
        // We are the only one who can dereference pointers right now.
        match state.keys.entry(key) {
            Entry::Occupied(entry) => {
                {
                    let node = unsafe { &mut *ptr };
                    let existing_ptr = entry.get();
                    // SAFETY(rescrv):
                    // We are the only one who can dereference pointers right now.
                    let existing_node = unsafe { &mut **existing_ptr };
                    std::mem::swap(&mut node.value, &mut existing_node.value);
                    state.size += existing_node.value.approximate_size();
                    state.size -= node.value.approximate_size();
                }
                // SAFETY(rescrv): We won't access ptr after this access.
                unsafe {
                    Node::drop(ptr);
                }
            }
            Entry::Vacant(entry) => {
                let node = unsafe { &mut *ptr };
                entry.insert(ptr);
                state.size += node.value.approximate_size();
                if state.head.is_null() {
                    assert_eq!(std::ptr::null_mut(), state.tail);
                    node.next = std::ptr::null_mut();
                    node.prev = std::ptr::null_mut();
                    state.head = ptr;
                    state.tail = ptr;
                } else {
                    node.next = state.head;
                    node.prev = std::ptr::null_mut();
                    // SAFETY(rescrv):  We know ptr and head are not aliased, so it is safe to
                    // bring this reference into existence.  No one else is derefing pointers.
                    let head = unsafe { &mut *state.head };
                    head.prev = ptr;
                    state.head = ptr;
                }
            }
        }
        state
    }

    /// Remove K if it exists.
    pub fn remove(&self, key: &K) {
        // SAFETY(rescrv):  Mutex poisoning.
        let mut state = self.state.lock().unwrap();
        if let Some(ptr) = state.keys.get(key) {
            let ptr = *ptr;
            // SAFETY(rescrv):  These functions are safe because we hold the mutex and no
            // outstanding references to data.
            unsafe {
                state = self.move_lru_to_back(state, ptr);
                drop(self.remove_lru(state));
            }
        }
    }

    /// Lookup K if it exists.
    pub fn lookup(&self, key: &K) -> Option<V> {
        let state = self.state.lock().unwrap();
        let ptr = *state.keys.get(key)?;
        let value = {
            // SAFETY(rescrv):  Mutual exclusion of state guarantees no one else will make a reference.
            let node = unsafe { &mut *ptr };
            node.value.clone()
        };
        // SAFETY(rescrv):  We hold no references across this call.
        unsafe {
            self.move_lru_to_front(state, ptr);
        }
        Some(value)
    }

    /// Pop (K, V) from the LRU, if there is one.
    pub fn pop(&self) -> Option<(K, V)> {
        let mut state = self.state.lock().unwrap();
        if !state.tail.is_null() {
            let (k, v) = {
                // SAFETY(rescrv):  We do this under a block and when we're the one holding the
                // lock.  That means we're the one who dereferences pointers.
                let tail = unsafe { &mut *state.tail };
                (tail.key.clone(), tail.value.clone())
            };
            // SAFETY(rescrv):  This is safe because we hold no references to tail.
            unsafe {
                drop(self.remove_lru(state));
            }
            Some((k, v))
        } else {
            None
        }
    }

    // The caller must make sure no references to linked nodes remain.
    unsafe fn remove_lru<'a>(
        &self,
        mut state: MutexGuard<'a, State<K, V>>,
    ) -> MutexGuard<'a, State<K, V>> {
        if !state.tail.is_null() {
            assert_ne!(std::ptr::null_mut(), state.head);
            let ptr = state.tail;
            // Do this in a block so the reference `tail` will go out of scope before the drop.
            {
                // SAFETY(rescrv):  The caller guarantees no nodes in the list are available as &Node
                // or &mut Node, so we can do this deref.
                let tail = &mut *ptr;
                if !tail.prev.is_null() {
                    // SAFETY(rescrv):  We know this node is different, so the same argument applies.
                    let new_tail = &mut *tail.prev;
                    new_tail.next = std::ptr::null_mut();
                }
                state.size -= tail.value.approximate_size();
                state.tail = tail.prev;
                if state.tail.is_null() {
                    state.head = std::ptr::null_mut();
                }
                tail.prev = std::ptr::null_mut();
                state.keys.remove(&tail.key);
            }
            // SAFETY(rescrv):  We dropped our reference to ptr, which was tail, and we know it to
            // be non-null by the if condition at the top.
            Node::drop(ptr);
        }
        state
    }

    // The caller must make sure no references to linked nodes remain.
    unsafe fn move_lru_to_front(
        &self,
        mut state: MutexGuard<'_, State<K, V>>,
        ptr: *mut Node<K, V>,
    ) {
        if ptr != state.head {
            // SAFETY(rescrv):  No references exist outside this function, and this is our first.
            let node = &mut *ptr;
            // SAFETY(rescrv):  We are not the head, so there must be a prev to adapt.
            assert_ne!(std::ptr::null_mut(), node.prev);
            let prev = &mut *node.prev;
            prev.next = node.next;
            if !node.next.is_null() {
                let next = &mut *node.next;
                next.prev = node.prev;
            } else {
                // SAFETY(rescrv):  Our next is none, so we must be the tail.
                assert_eq!(state.tail, ptr);
                state.tail = node.prev;
            }
            node.next = state.head;
            node.prev = std::ptr::null_mut();
            // SAFETY(rescrv):  We know ptr and head are not aliased, so it is safe to
            // bring this reference into existence.  No one else is derefing pointers.
            let head = unsafe { &mut *state.head };
            head.prev = ptr;
            state.head = ptr;
        }
    }

    // The caller must make sure no references to linked nodes remain.
    unsafe fn move_lru_to_back<'a>(
        &self,
        mut state: MutexGuard<'a, State<K, V>>,
        ptr: *mut Node<K, V>,
    ) -> MutexGuard<'a, State<K, V>> {
        if ptr != state.tail {
            // SAFETY(rescrv):  No references exist outside this function, and this is our first.
            let node = &mut *ptr;
            // SAFETY(rescrv):  We are not the tail, so there must be a next to adapt.
            assert_ne!(std::ptr::null_mut(), node.next);
            let next = &mut *node.next;
            next.prev = node.prev;
            if !node.prev.is_null() {
                let prev = &mut *node.prev;
                prev.next = node.next;
            } else {
                // SAFETY(rescrv):  Our prev is none, so we must be the head.
                assert_eq!(state.head, ptr);
                state.head = node.next;
            }
            node.prev = state.tail;
            node.next = std::ptr::null_mut();
            // SAFETY(rescrv):  We know ptr and head are not aliased, so it is safe to
            // bring this reference into existence.  No one else is derefing pointers.
            let tail = unsafe { &mut *state.tail };
            tail.next = ptr;
            state.tail = ptr;
        }
        state
    }
}

impl<K: Clone + Eq + Hash, V: Value> Drop for LeastRecentlyUsedCache<K, V> {
    fn drop(&mut self) {
        let mut state = self.state.lock().unwrap();
        while !state.head.is_null() {
            // SAFETY(rescrv):  We only held a reference to `node` and we dropped it before this.
            state = unsafe { self.remove_lru(state) };
        }
    }
}

unsafe impl<K: Clone + Eq + Hash + Send + Sync, V: Value + Send + Sync> Send
    for LeastRecentlyUsedCache<K, V>
{
}
unsafe impl<K: Clone + Eq + Hash + Send + Sync, V: Value + Send + Sync> Sync
    for LeastRecentlyUsedCache<K, V>
{
}

/////////////////////////////////////////////// State //////////////////////////////////////////////

struct State<K: Clone + Eq + Hash, V: Value> {
    size: usize,
    head: *mut Node<K, V>,
    tail: *mut Node<K, V>,
    keys: HashMap<K, *mut Node<K, V>>,
}

/////////////////////////////////////////////// Node ///////////////////////////////////////////////

struct Node<K: Clone + Eq + Hash, V: Value> {
    next: *mut Node<K, V>,
    prev: *mut Node<K, V>,
    key: K,
    value: V,
}

impl<K: Clone + Eq + Hash, V: Value> Node<K, V> {
    fn new(key: K, value: V) -> *mut Node<K, V> {
        Box::leak(Box::new(Self {
            next: std::ptr::null_mut(),
            prev: std::ptr::null_mut(),
            key,
            value,
        }))
    }

    unsafe fn drop(node: *mut Node<K, V>) {
        drop(Box::from_raw(node));
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    use guacamole::{FromGuacamole, Guacamole};

    use super::*;

    #[test]
    fn empty() {
        let _lru = LeastRecentlyUsedCache::<String, String>::new(16);
    }

    #[test]
    fn insert_one() {
        let lru = LeastRecentlyUsedCache::<String, String>::new(128);
        lru.insert("Hello".to_string(), "World".to_string());
        assert_eq!(Some("World".to_string()), lru.lookup(&"Hello".to_string()));
    }

    #[test]
    fn bump_one() {
        let lru = LeastRecentlyUsedCache::<String, String>::new(6);
        lru.insert("Hello".to_string(), "World".to_string());
        assert_eq!(Some("World".to_string()), lru.lookup(&"Hello".to_string()));
        lru.insert("Goodbye".to_string(), "World".to_string());
        assert_eq!(None, lru.lookup(&"Hello".to_string()));
        assert_eq!(
            Some("World".to_string()),
            lru.lookup(&"Goodbye".to_string())
        );
    }

    fn guacamole_thread(
        lru: Arc<LeastRecentlyUsedCache<u64, u64>>,
        seed: u64,
        count: Arc<AtomicU64>,
    ) {
        let mut guac = Guacamole::new(seed);
        while count.load(Ordering::Relaxed) < 100_000 {
            let k = u64::from_guacamole(&mut (), &mut guac);
            let v = u64::from_guacamole(&mut (), &mut guac);
            lru.insert(k, v);
            lru.lookup(&k);
            count.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn guacamole(seed: u64) {
        let lru = Arc::new(LeastRecentlyUsedCache::new(1_000));
        let mut guac = Guacamole::new(seed);
        let num_threads = usize::from_guacamole(&mut (), &mut guac) % 32;
        let count = Arc::new(AtomicU64::new(0));
        let mut threads = Vec::with_capacity(num_threads);
        for _ in 0..num_threads {
            let l = Arc::clone(&lru);
            let s = u64::from_guacamole(&mut (), &mut guac);
            let c = Arc::clone(&count);
            threads.push(std::thread::spawn(move || guacamole_thread(l, s, c)));
        }
        for thread in threads.into_iter() {
            thread.join().unwrap();
        }
    }

    #[test]
    fn guacamole6643963873287725700() {
        guacamole(6643963873287725700)
    }

    #[test]
    fn guacamole7137897304743841561() {
        guacamole(7137897304743841561)
    }

    #[test]
    fn guacamole2317217670142823873() {
        guacamole(2317217670142823873)
    }

    #[test]
    fn guacamole11329384696052517592() {
        guacamole(11329384696052517592)
    }

    #[test]
    fn guacamole18118936442608574906() {
        guacamole(18118936442608574906)
    }

    #[test]
    fn guacamole15718820984497662670() {
        guacamole(15718820984497662670)
    }

    #[test]
    fn guacamole17917937518970327178() {
        guacamole(17917937518970327178)
    }

    #[test]
    fn guacamole10255838224480336524() {
        guacamole(10255838224480336524)
    }

    #[test]
    fn guacamole246982374299724949() {
        guacamole(246982374299724949)
    }

    #[test]
    fn guacamole6639946947205542247() {
        guacamole(6639946947205542247)
    }

    #[test]
    fn guacamole8699617108035283357() {
        guacamole(8699617108035283357)
    }

    #[test]
    fn guacamole324460569595836317() {
        guacamole(324460569595836317)
    }

    #[test]
    fn guacamole14141400077699195241() {
        guacamole(14141400077699195241)
    }

    #[test]
    fn guacamole8067398835850766806() {
        guacamole(8067398835850766806)
    }

    #[test]
    fn guacamole4944978611314439769() {
        guacamole(4944978611314439769)
    }

    #[test]
    fn guacamole14383940742507881265() {
        guacamole(14383940742507881265)
    }

    #[test]
    fn guacamole16377450408782845911() {
        guacamole(16377450408782845911)
    }

    #[test]
    fn guacamole6440572597074903414() {
        guacamole(6440572597074903414)
    }

    #[test]
    fn guacamole6518552131696804795() {
        guacamole(6518552131696804795)
    }

    #[test]
    fn guacamole3303319316588366166() {
        guacamole(3303319316588366166)
    }

    #[test]
    fn guacamole4017360561893399133() {
        guacamole(4017360561893399133)
    }

    #[test]
    fn guacamole9065699422885789108() {
        guacamole(9065699422885789108)
    }

    #[test]
    fn guacamole10768333185320367541() {
        guacamole(10768333185320367541)
    }

    #[test]
    fn guacamole14652712928456270161() {
        guacamole(14652712928456270161)
    }

    #[test]
    fn guacamole14673962558856575051() {
        guacamole(14673962558856575051)
    }

    #[test]
    fn guacamole7324466076719006097() {
        guacamole(7324466076719006097)
    }

    #[test]
    fn guacamole3320509479511817219() {
        guacamole(3320509479511817219)
    }

    #[test]
    fn guacamole14591100764259745984() {
        guacamole(14591100764259745984)
    }

    #[test]
    fn guacamole80143373126134956() {
        guacamole(80143373126134956)
    }

    #[test]
    fn guacamole14254437585891870357() {
        guacamole(14254437585891870357)
    }

    #[test]
    fn guacamole1877611980528522935() {
        guacamole(1877611980528522935)
    }

    #[test]
    fn guacamole10516678793248279248() {
        guacamole(10516678793248279248)
    }

    #[test]
    fn guacamole2208789614482528524() {
        guacamole(2208789614482528524)
    }

    #[test]
    fn guacamole3626616070959083137() {
        guacamole(3626616070959083137)
    }

    #[test]
    fn guacamole1143643537273625111() {
        guacamole(1143643537273625111)
    }

    #[test]
    fn guacamole2733723355657003561() {
        guacamole(2733723355657003561)
    }

    #[test]
    fn guacamole3597577870835358410() {
        guacamole(3597577870835358410)
    }

    #[test]
    fn guacamole9907478478060667830() {
        guacamole(9907478478060667830)
    }

    #[test]
    fn guacamole12755452682312088528() {
        guacamole(12755452682312088528)
    }

    #[test]
    fn guacamole12010789949984857857() {
        guacamole(12010789949984857857)
    }

    #[test]
    fn guacamole2172494012740847644() {
        guacamole(2172494012740847644)
    }

    #[test]
    fn guacamole4683773090215001530() {
        guacamole(4683773090215001530)
    }

    #[test]
    fn guacamole9607168360918661444() {
        guacamole(9607168360918661444)
    }

    #[test]
    fn guacamole5096222197520622318() {
        guacamole(5096222197520622318)
    }

    #[test]
    fn guacamole9558090827395204383() {
        guacamole(9558090827395204383)
    }

    #[test]
    fn guacamole5123112340922497158() {
        guacamole(5123112340922497158)
    }

    #[test]
    fn guacamole6061677557436624432() {
        guacamole(6061677557436624432)
    }

    #[test]
    fn guacamole825178995426522985() {
        guacamole(825178995426522985)
    }

    #[test]
    fn guacamole12682831657278419914() {
        guacamole(12682831657278419914)
    }

    #[test]
    fn guacamole8053266640118673186() {
        guacamole(8053266640118673186)
    }

    #[test]
    fn guacamole9774843062374528120() {
        guacamole(9774843062374528120)
    }

    #[test]
    fn guacamole15342157449597448152() {
        guacamole(15342157449597448152)
    }

    #[test]
    fn guacamole6616032404529850977() {
        guacamole(6616032404529850977)
    }

    #[test]
    fn guacamole7402589127483390035() {
        guacamole(7402589127483390035)
    }

    #[test]
    fn guacamole1790232934390704448() {
        guacamole(1790232934390704448)
    }

    #[test]
    fn guacamole13897475022589417323() {
        guacamole(13897475022589417323)
    }

    #[test]
    fn guacamole11131097472240026722() {
        guacamole(11131097472240026722)
    }

    #[test]
    fn guacamole9238977550597094952() {
        guacamole(9238977550597094952)
    }

    #[test]
    fn guacamole4158217929443476850() {
        guacamole(4158217929443476850)
    }

    #[test]
    fn guacamole6329028112560807121() {
        guacamole(6329028112560807121)
    }

    #[test]
    fn guacamole17196344605709572875() {
        guacamole(17196344605709572875)
    }

    #[test]
    fn guacamole17949739483993176455() {
        guacamole(17949739483993176455)
    }

    #[test]
    fn guacamole8242885838749006031() {
        guacamole(8242885838749006031)
    }

    #[test]
    fn guacamole11041737309993626710() {
        guacamole(11041737309993626710)
    }

    #[test]
    fn guacamole8596857132683877016() {
        guacamole(8596857132683877016)
    }

    #[test]
    fn guacamole5028419926425315651() {
        guacamole(5028419926425315651)
    }

    #[test]
    fn guacamole8823831865473308745() {
        guacamole(8823831865473308745)
    }

    #[test]
    fn guacamole5852791434107298260() {
        guacamole(5852791434107298260)
    }

    #[test]
    fn guacamole6420539549041764220() {
        guacamole(6420539549041764220)
    }

    #[test]
    fn guacamole14877617924499253175() {
        guacamole(14877617924499253175)
    }

    #[test]
    fn guacamole10359632481723088944() {
        guacamole(10359632481723088944)
    }

    #[test]
    fn guacamole5798265307709855298() {
        guacamole(5798265307709855298)
    }

    #[test]
    fn guacamole6845171597970930451() {
        guacamole(6845171597970930451)
    }

    #[test]
    fn guacamole10123668592680715424() {
        guacamole(10123668592680715424)
    }

    #[test]
    fn guacamole6817053893763286600() {
        guacamole(6817053893763286600)
    }

    #[test]
    fn guacamole5043755100919235570() {
        guacamole(5043755100919235570)
    }

    #[test]
    fn guacamole13767829340339947296() {
        guacamole(13767829340339947296)
    }

    #[test]
    fn guacamole15272518372471738484() {
        guacamole(15272518372471738484)
    }

    #[test]
    fn guacamole5975732403272375652() {
        guacamole(5975732403272375652)
    }

    #[test]
    fn guacamole11371456206423251316() {
        guacamole(11371456206423251316)
    }

    #[test]
    fn guacamole2989692141685624374() {
        guacamole(2989692141685624374)
    }

    #[test]
    fn guacamole5713319554772393873() {
        guacamole(5713319554772393873)
    }

    #[test]
    fn guacamole12097348388461543202() {
        guacamole(12097348388461543202)
    }

    #[test]
    fn guacamole5176147892203871809() {
        guacamole(5176147892203871809)
    }

    #[test]
    fn guacamole17755308676966593242() {
        guacamole(17755308676966593242)
    }

    #[test]
    fn guacamole10073086343966704032() {
        guacamole(10073086343966704032)
    }

    #[test]
    fn guacamole6054264604825596291() {
        guacamole(6054264604825596291)
    }

    #[test]
    fn guacamole11210303247299960443() {
        guacamole(11210303247299960443)
    }

    #[test]
    fn guacamole17914957350531144410() {
        guacamole(17914957350531144410)
    }

    #[test]
    fn guacamole3201757213583981843() {
        guacamole(3201757213583981843)
    }

    #[test]
    fn guacamole5931287646214908170() {
        guacamole(5931287646214908170)
    }

    #[test]
    fn guacamole16504430361537631561() {
        guacamole(16504430361537631561)
    }

    #[test]
    fn guacamole658348099974014570() {
        guacamole(658348099974014570)
    }

    #[test]
    fn guacamole10280193607363353543() {
        guacamole(10280193607363353543)
    }

    #[test]
    fn guacamole3836373529551491966() {
        guacamole(3836373529551491966)
    }

    #[test]
    fn guacamole3907397257238114082() {
        guacamole(3907397257238114082)
    }

    #[test]
    fn guacamole7953136082766321936() {
        guacamole(7953136082766321936)
    }

    #[test]
    fn guacamole12540079778734656791() {
        guacamole(12540079778734656791)
    }

    #[test]
    fn guacamole7281899546506092909() {
        guacamole(7281899546506092909)
    }

    #[test]
    fn guacamole12694955016238954511() {
        guacamole(12694955016238954511)
    }

    #[test]
    fn guacamole11623220113949101239() {
        guacamole(11623220113949101239)
    }

    #[test]
    fn guacamole16375772096015101582() {
        guacamole(16375772096015101582)
    }

    #[test]
    fn guacamole3478615096064275855() {
        guacamole(3478615096064275855)
    }

    #[test]
    fn guacamole7767428294873306778() {
        guacamole(7767428294873306778)
    }

    #[test]
    fn guacamole5700240193932540843() {
        guacamole(5700240193932540843)
    }

    #[test]
    fn guacamole12023783631624717854() {
        guacamole(12023783631624717854)
    }

    #[test]
    fn guacamole3670921603004318553() {
        guacamole(3670921603004318553)
    }

    #[test]
    fn guacamole7115477186655918266() {
        guacamole(7115477186655918266)
    }

    #[test]
    fn guacamole7538002874666812588() {
        guacamole(7538002874666812588)
    }

    #[test]
    fn guacamole13143166734511941613() {
        guacamole(13143166734511941613)
    }

    #[test]
    fn guacamole16438116022511643404() {
        guacamole(16438116022511643404)
    }

    #[test]
    fn guacamole10228727419760680423() {
        guacamole(10228727419760680423)
    }

    #[test]
    fn guacamole9732174417319049224() {
        guacamole(9732174417319049224)
    }

    #[test]
    fn guacamole8337810437742124651() {
        guacamole(8337810437742124651)
    }

    #[test]
    fn guacamole6865868502180192748() {
        guacamole(6865868502180192748)
    }

    #[test]
    fn guacamole15667301855375675291() {
        guacamole(15667301855375675291)
    }

    #[test]
    fn guacamole7008015784393062874() {
        guacamole(7008015784393062874)
    }

    #[test]
    fn guacamole8528053684996466666() {
        guacamole(8528053684996466666)
    }

    #[test]
    fn guacamole8762180490090364255() {
        guacamole(8762180490090364255)
    }

    #[test]
    fn guacamole17950708367571068540() {
        guacamole(17950708367571068540)
    }

    #[test]
    fn guacamole4384035186212511325() {
        guacamole(4384035186212511325)
    }

    #[test]
    fn guacamole17537754167396976555() {
        guacamole(17537754167396976555)
    }

    #[test]
    fn guacamole3902250231851721388() {
        guacamole(3902250231851721388)
    }

    #[test]
    fn guacamole18387901026806482735() {
        guacamole(18387901026806482735)
    }

    #[test]
    fn guacamole7429035785165093816() {
        guacamole(7429035785165093816)
    }

    #[test]
    fn guacamole10083837366657116843() {
        guacamole(10083837366657116843)
    }

    #[test]
    fn guacamole16582874771946804379() {
        guacamole(16582874771946804379)
    }

    #[test]
    fn guacamole7895768985087893597() {
        guacamole(7895768985087893597)
    }

    #[test]
    fn guacamole1279265844260850297() {
        guacamole(1279265844260850297)
    }

    #[test]
    fn guacamole8455744569416988604() {
        guacamole(8455744569416988604)
    }

    #[test]
    fn guacamole1062352881320545268() {
        guacamole(1062352881320545268)
    }

    #[test]
    fn guacamole5115362495493032835() {
        guacamole(5115362495493032835)
    }

    #[test]
    fn guacamole15411918768574122096() {
        guacamole(15411918768574122096)
    }

    #[test]
    fn guacamole17502305924622991413() {
        guacamole(17502305924622991413)
    }

    #[test]
    fn guacamole7525602278946761472() {
        guacamole(7525602278946761472)
    }

    #[test]
    fn guacamole11361176628272461779() {
        guacamole(11361176628272461779)
    }

    #[test]
    fn guacamole7762509103363396504() {
        guacamole(7762509103363396504)
    }
}
