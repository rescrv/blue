//! A lock-free skip-list.
// A transcription of this code from hyperleveldb.
//
// That code, in turn, comes from LevelDB which has this copyright claim:
//
// Copyright (c) 2011 The LevelDB Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file. See the AUTHORS file for names of contributors.

use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Arc;

use rand::Rng;

const DEFAULT_MAX_HEIGHT: usize = 12;

/////////////////////////////////////////////// Node ///////////////////////////////////////////////

struct Node<K, V, const MAX_HEIGHT: usize = DEFAULT_MAX_HEIGHT> {
    key: K,
    value: V,
    pointers: Vec<AtomicPtr<Node<K, V, MAX_HEIGHT>>>,
}

impl<K, V, const MAX_HEIGHT: usize> Node<K, V, MAX_HEIGHT> {
    fn new(key: K, value: V, height: usize) -> Self {
        let mut pointers = Vec::with_capacity(height);
        for _ in 0..height {
            pointers.push(AtomicPtr::new(std::ptr::null_mut()));
        }
        Self {
            key,
            value,
            pointers,
        }
    }

    fn set_next(&self, level: usize, x: *mut Node<K, V, MAX_HEIGHT>) {
        assert!(level < self.pointers.len());
        self.pointers[level].store(x, Ordering::Release);
    }

    fn get_next(&self, level: usize) -> *mut Node<K, V, MAX_HEIGHT> {
        assert!(level < self.pointers.len());
        self.pointers[level].load(Ordering::Acquire)
    }

    fn cas_next(
        &self,
        level: usize,
        old_node: *mut Node<K, V, MAX_HEIGHT>,
        new_node: *mut Node<K, V, MAX_HEIGHT>,
    ) -> bool {
        assert!(level < self.pointers.len());
        self.pointers[level].compare_exchange(
            old_node,
            new_node,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) == Ok(old_node)
    }
}

mod node_ptr {
    use super::Node;

    fn deref<'a, K, V, const MAX_HEIGHT: usize>(
        ptr: *mut Node<K, V, MAX_HEIGHT>,
    ) -> &'a Node<K, V, MAX_HEIGHT> {
        unsafe { &*ptr }
    }

    pub(crate) fn key<'a, K: 'a, V: 'a, const MAX_HEIGHT: usize>(
        ptr: *mut Node<K, V, MAX_HEIGHT>,
    ) -> &'a K {
        &deref(ptr).key
    }

    pub(crate) fn value<'a, K: 'a, V: 'a, const MAX_HEIGHT: usize>(
        ptr: *mut Node<K, V, MAX_HEIGHT>,
    ) -> &'a V {
        &deref(ptr).value
    }

    pub(crate) fn set_next<K, V, const MAX_HEIGHT: usize>(
        ptr: *mut Node<K, V, MAX_HEIGHT>,
        level: usize,
        next: *mut Node<K, V, MAX_HEIGHT>,
    ) {
        deref(ptr).set_next(level, next);
    }

    pub(crate) fn get_next<K, V, const MAX_HEIGHT: usize>(
        ptr: *mut Node<K, V, MAX_HEIGHT>,
        level: usize,
    ) -> *mut Node<K, V, MAX_HEIGHT> {
        deref(ptr).get_next(level)
    }

    pub(crate) fn cas_next<K, V, const MAX_HEIGHT: usize>(
        ptr: *mut Node<K, V, MAX_HEIGHT>,
        level: usize,
        old_node: *mut Node<K, V, MAX_HEIGHT>,
        new_node: *mut Node<K, V, MAX_HEIGHT>,
    ) -> bool {
        deref(ptr).cas_next(level, old_node, new_node)
    }
}

///////////////////////////////////////////// SkipList /////////////////////////////////////////////

/// A lock-free skip list, generic over keys and values.
pub struct SkipList<K, V, const MAX_HEIGHT: usize = DEFAULT_MAX_HEIGHT> {
    head: Arc<AtomicPtr<Node<K, V, MAX_HEIGHT>>>,
}

impl<K: Eq + Ord + Default, V: Default, const MAX_HEIGHT: usize> SkipList<K, V, MAX_HEIGHT> {
    /// Insert the provided key and value into the skiplist.
    pub fn insert(&self, key: K, value: V) {
        let (existing, mut prev, mut obs) =
            Self::find_greater_or_equal_and_pointers(&self.head, &key);
        assert!(existing.is_null() || node_ptr::key(existing) != &key);
        let height = Self::random_height();
        let x = Self::new_node(key, value, height);
        for idx in 0..height {
            'lockfree_looping: loop {
                node_ptr::set_next(x, idx, obs[idx]);
                if node_ptr::cas_next(prev[idx], idx, obs[idx], x) {
                    break 'lockfree_looping;
                }
                'advancing: loop {
                    let next = node_ptr::get_next(prev[idx], idx);
                    if Self::key_is_after_node(node_ptr::key(x), next) {
                        prev[idx] = next;
                    } else {
                        obs[idx] = next;
                        break 'advancing;
                    }
                }
            }
        }
    }

    /// True iff the skiplist contains `key`.
    pub fn contains(&self, key: &K) -> bool {
        let x = Self::find_greater_or_equal(&self.head, key);
        !x.is_null() && node_ptr::key(x) == key
    }

    /// Return a skiplist iterator.
    ///
    /// This iterator will keep the body of the skiplist in-memory even after the skiplist itself
    /// goes out of scope.
    pub fn iter(&self) -> SkipListIterator<K, V, MAX_HEIGHT> {
        SkipListIterator {
            head: Arc::clone(&self.head),
            node: std::ptr::null_mut(),
        }
    }

    fn new_node(key: K, value: V, height: usize) -> *mut Node<K, V, MAX_HEIGHT> {
        assert!(height > 0);
        assert!(height <= MAX_HEIGHT);
        Box::leak(Box::new(Node::new(key, value, height)))
    }

    fn random_height() -> usize {
        const BRANCHING: u8 = 4;
        let mut height = 1usize;
        let mut rng = rand::thread_rng();
        while height < MAX_HEIGHT && rng.gen::<u8>() % BRANCHING == 0 {
            height += 1;
        }
        assert!(height > 0);
        assert!(height <= MAX_HEIGHT);
        height
    }

    fn key_is_after_node(key: &K, node: *mut Node<K, V, MAX_HEIGHT>) -> bool {
        !node.is_null() && node_ptr::key(node) < key
    }

    fn find_greater_or_equal(
        head: &AtomicPtr<Node<K, V, MAX_HEIGHT>>,
        key: &K,
    ) -> *mut Node<K, V, MAX_HEIGHT> {
        let mut x = head.load(Ordering::Acquire);
        let mut level = MAX_HEIGHT - 1;
        loop {
            let next = node_ptr::get_next(x, level);
            if Self::key_is_after_node(key, next) {
                x = next;
            } else if level == 0 {
                return next;
            } else {
                level -= 1;
            }
        }
    }

    // NOTE(rescrv):  I don't want a simpler type.  I want a 3-tuple that I can destructure.
    #[allow(clippy::type_complexity)]
    fn find_greater_or_equal_and_pointers(
        head: &AtomicPtr<Node<K, V, MAX_HEIGHT>>,
        key: &K,
    ) -> (
        *mut Node<K, V, MAX_HEIGHT>,
        Vec<*mut Node<K, V, MAX_HEIGHT>>,
        Vec<*mut Node<K, V, MAX_HEIGHT>>,
    ) {
        let mut x = head.load(Ordering::Acquire);
        let mut level = MAX_HEIGHT - 1;
        let mut prev = Vec::with_capacity(MAX_HEIGHT);
        let mut obs = Vec::with_capacity(MAX_HEIGHT);
        for _ in 0..MAX_HEIGHT {
            prev.push(std::ptr::null_mut());
            obs.push(std::ptr::null_mut());
        }
        let found = loop {
            let next = node_ptr::get_next(x, level);
            if Self::key_is_after_node(key, next) {
                x = next;
            } else {
                prev[level] = x;
                obs[level] = next;
                if level == 0 {
                    break next;
                } else {
                    level -= 1;
                }
            }
        };
        (found, prev, obs)
    }

    fn find_less_than(
        head: &AtomicPtr<Node<K, V, MAX_HEIGHT>>,
        key: &K,
    ) -> *mut Node<K, V, MAX_HEIGHT> {
        let mut x = head.load(Ordering::Acquire);
        let mut level = MAX_HEIGHT - 1;
        loop {
            assert!(std::ptr::eq(x, head.load(Ordering::Relaxed)) || node_ptr::key(x) < key);
            let next = node_ptr::get_next(x, level);
            if next.is_null() || node_ptr::key(next) >= key {
                if level == 0 {
                    return x;
                } else {
                    level -= 1;
                }
            } else {
                x = next;
            }
        }
    }

    fn find_last(head: &AtomicPtr<Node<K, V, MAX_HEIGHT>>) -> *mut Node<K, V, MAX_HEIGHT> {
        let mut x = head.load(Ordering::Acquire);
        let mut level = MAX_HEIGHT - 1;
        loop {
            let next = node_ptr::get_next(x, level);
            if next.is_null() {
                if level == 0 {
                    return x;
                } else {
                    level -= 1;
                }
            } else {
                x = next;
            }
        }
    }
}

impl<K: Eq + Ord + Default, V: Default, const MAX_HEIGHT: usize> Default
    for SkipList<K, V, MAX_HEIGHT>
{
    fn default() -> Self {
        let head = Self::new_node(K::default(), V::default(), MAX_HEIGHT);
        for idx in 0..MAX_HEIGHT {
            node_ptr::set_next(head, idx, std::ptr::null_mut());
        }
        let head = Arc::new(AtomicPtr::new(head));
        Self { head }
    }
}

impl<K, V, const MAX_HEIGHT: usize> Drop for SkipList<K, V, MAX_HEIGHT> {
    fn drop(&mut self) {
        let mut ptr = self.head.load(Ordering::Acquire);
        while !ptr.is_null() {
            let to_drop = ptr;
            ptr = node_ptr::get_next(ptr, 0);
            drop(unsafe { Box::from_raw(to_drop) });
        }
    }
}

///////////////////////////////////////// SkipListIterator /////////////////////////////////////////

/// A SkipList iterator.  Will outlast the skip list it comes from if so chosen.
#[derive(Clone)]
pub struct SkipListIterator<K, V, const MAX_HEIGHT: usize = DEFAULT_MAX_HEIGHT> {
    head: Arc<AtomicPtr<Node<K, V, MAX_HEIGHT>>>,
    node: *mut Node<K, V, MAX_HEIGHT>,
}

impl<K: Eq + Ord + Default, V: Default, const MAX_HEIGHT: usize>
    SkipListIterator<K, V, MAX_HEIGHT>
{
    /// Returns true if the skip list is positioned at a key-value pair.
    pub fn is_valid(&self) -> bool {
        !self.node.is_null() && self.node != self.head.load(Ordering::Relaxed)
    }

    /// The key the skiplist is positioned to.
    ///
    /// # Panics
    ///
    /// Will panic if the skiplist is not valid.
    pub fn key(&self) -> &K {
        assert!(self.is_valid());
        node_ptr::key(self.node)
    }

    /// The value the skiplist is positioned to.
    ///
    /// # Panics
    ///
    /// Will panic if the skiplist is not valid.
    pub fn value(&self) -> &V {
        assert!(self.is_valid());
        node_ptr::value(self.node)
    }

    /// Advance forward in the skip list, to the next greater key.
    pub fn next(&mut self) {
        if !self.node.is_null() {
            self.node = node_ptr::get_next(self.node, 0);
        }
    }

    /// Advance forward in the skip list, to the next smaller key.
    pub fn prev(&mut self) {
        if self.node.is_null() {
            self.node = SkipList::find_last(&self.head);
        } else if self.node != self.head.load(Ordering::Relaxed) {
            self.node = SkipList::find_less_than(&self.head, node_ptr::key(self.node));
        }
    }

    /// Seek to the first key >= the provided key.
    pub fn seek(&mut self, key: &K) {
        self.node = SkipList::find_greater_or_equal(&self.head, key);
    }

    /// Seek to the empty start of the skiplist.
    ///
    /// After this call the skiplist will not be valid.  Call next to get the next node.
    pub fn seek_to_first(&mut self) {
        self.node = node_ptr::get_next(self.head.load(Ordering::Acquire), 0);
    }

    /// Seek to the empty end of the skiplist.
    ///
    /// After this call the skiplist will not be valid.  Call prev to get the prev node.
    pub fn seek_to_last(&mut self) {
        self.node = std::ptr::null_mut();
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use guacamole::{FromGuacamole, Guacamole};

    use super::SkipList;

    #[test]
    fn empty() {
        let sl = SkipList::<u64, u64>::default();
        let iter = sl.iter();
        assert!(!iter.is_valid());
    }

    #[test]
    fn one_two_three() {
        let sl = SkipList::<u64, u64>::default();
        sl.insert(1, 42);
        sl.insert(2, 84);
        sl.insert(3, 126);
        let mut iter = sl.iter();
        iter.seek_to_first();
        // 1
        assert!(iter.is_valid());
        assert_eq!(1, *iter.key());
        // 2
        iter.next();
        assert!(iter.is_valid());
        assert_eq!(2, *iter.key());
        // 3
        iter.next();
        assert!(iter.is_valid());
        assert_eq!(3, *iter.key());
        // Invalid
        iter.next();
        assert!(!iter.is_valid());
    }

    #[test]
    fn late_insert() {
        let sl = SkipList::<u64, u64>::default();
        sl.insert(1, 42);
        sl.insert(2, 84);
        sl.insert(3, 126);
        let mut iter = sl.iter();
        iter.seek_to_first();
        // 1
        assert!(iter.is_valid());
        assert_eq!(1, *iter.key());
        // 2
        iter.next();
        assert!(iter.is_valid());
        assert_eq!(2, *iter.key());
        // 3
        iter.next();
        assert!(iter.is_valid());
        assert_eq!(3, *iter.key());
        // 4
        sl.insert(4, 168);
        iter.next();
        assert!(iter.is_valid());
        assert_eq!(4, *iter.key());
        // done
        iter.next();
        assert!(!iter.is_valid());
    }

    #[test]
    fn reverse_reverse() {
        let sl = SkipList::<u64, u64>::default();
        sl.insert(1, 42);
        sl.insert(2, 84);
        sl.insert(3, 126);
        let mut iter = sl.iter();
        iter.seek_to_last();
        assert!(!iter.is_valid());
        // 3
        iter.prev();
        assert!(iter.is_valid());
        assert_eq!(3, *iter.key());
        // 2
        iter.prev();
        assert!(iter.is_valid());
        assert_eq!(2, *iter.key());
        // 1
        iter.prev();
        assert!(iter.is_valid());
        assert_eq!(1, *iter.key());
        // done
        iter.prev();
        assert!(!iter.is_valid());
    }

    fn guacamole_writer(skiplist: Arc<SkipList<u64, u64>>, seed: u64) {
        let mut guac = Guacamole::new(seed);
        for _ in 0..10_000 {
            let k = u64::from_guacamole(&mut (), &mut guac);
            let v = u64::from_guacamole(&mut (), &mut guac);
            skiplist.insert(k, v);
        }
    }

    fn guacamole_reader(skiplist: Arc<SkipList<u64, u64>>, shutdown: Arc<AtomicBool>) {
        while !shutdown.load(Ordering::Relaxed) {
            let mut iter = skiplist.iter();
            iter.seek_to_first();
            let mut prev = None;
            while iter.is_valid() {
                if let Some(prev) = prev {
                    assert!(prev <= *iter.key());
                }
                prev = Some(*iter.key());
                iter.next();
            }
        }
    }

    fn guacamole(seed: u64) {
        let skiplist = Arc::new(SkipList::default());
        let mut guac = Guacamole::new(seed);
        let readers = u64::from_guacamole(&mut (), &mut guac) % 16;
        let writers = u64::from_guacamole(&mut (), &mut guac) % 4;
        let shutdown_signal = Arc::new(AtomicBool::new(false));
        let mut reader_threads = Vec::with_capacity(readers as usize);
        for _ in 0..readers {
            let s = Arc::clone(&shutdown_signal);
            let sl = Arc::clone(&skiplist);
            reader_threads.push(std::thread::spawn(move || guacamole_reader(sl, s)));
        }
        let mut writer_threads = Vec::with_capacity(writers as usize);
        for _ in 0..writers {
            let sl = Arc::clone(&skiplist);
            let seed = u64::from_guacamole(&mut (), &mut guac);
            writer_threads.push(std::thread::spawn(move || {
                guacamole_writer(sl, seed);
            }));
        }
        for writer in writer_threads.into_iter() {
            writer.join().unwrap();
        }
        shutdown_signal.store(true, Ordering::Relaxed);
        for reader in reader_threads.into_iter() {
            reader.join().unwrap();
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
