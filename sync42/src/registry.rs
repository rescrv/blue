//! A global registry of static objects.  Allows objects to belong to one or more registries.

use std::sync::atomic::{AtomicPtr, Ordering};

use biometrics::{Collector, Counter};

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static REGISTERED: Counter = Counter::new("sync42.registry.registered");

/// Register the biometrics for the registry
pub fn register_biometrics(collector: &Collector) {
    collector.register_counter(&REGISTERED);
}

///////////////////////////////////////////// Registry /////////////////////////////////////////////

/// A registry is a static object to which other static objects will attach with
/// `Registration::ensure_registered`.
#[derive(Debug)]
pub struct Registry<T> {
    list: AtomicPtr<Registration<T>>,
}

impl<T> Registry<T> {
    pub const fn new() -> Self {
        let list = AtomicPtr::new(std::ptr::null_mut());
        Self { list }
    }

    pub fn walk(&self) -> Walker<T> {
        Walker::new(self)
    }
}

impl<T> Default for Registry<T> {
    fn default() -> Self {
        Self::new()
    }
}

/////////////////////////////////////////// Registration ///////////////////////////////////////////

/// A registration is intended to be kept within a static object, so that every invocation of that
/// object can call `ensure_registered` to ensure that it's registered.  Registration the verb is
/// guaranteed to happen at most once per Registration the noun.
#[derive(Debug)]
pub struct Registration<T> {
    this: AtomicPtr<T>,
    next: AtomicPtr<Registration<T>>,
}

impl<T> Registration<T> {
    /// Construct a new Registration in const-fashion.
    pub const fn new() -> Self {
        let this = AtomicPtr::new(std::ptr::null_mut());
        let next = AtomicPtr::new(std::ptr::null_mut());
        Self { this, next }
    }

    /// Efficiently ensure that this statically-allocated registration is registered with the
    /// provided registry.  The provided t should be the object which registry-walkers will visit.
    #[inline(always)]
    pub fn ensure_registered(&'static self, registry: &Registry<T>, t: &'static T) {
        if self.this.load(Ordering::Relaxed).is_null() {
            self.ensure_registered_slow(registry, t);
        }
    }

    #[inline(never)]
    fn ensure_registered_slow(&'static self, registry: &Registry<T>, t: &'static T) {
        let t: *mut T = t as *const T as *mut T;
        // First "acquire" the right to link the value.
        if self
            .this
            .compare_exchange(std::ptr::null_mut(), t, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
        {
            return;
        }
        // Then, spin-loop to link it into the list.
        let this: *mut Registration<T> = self as *const Registration<T> as *mut Registration<T>;
        loop {
            let list = registry.list.load(Ordering::Relaxed);
            self.next.store(list, Ordering::Release);
            if registry
                .list
                .compare_exchange(list, this, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }
}

impl<T> Default for Registration<T> {
    fn default() -> Self {
        Self::new()
    }
}

////////////////////////////////////////////// Walker //////////////////////////////////////////////

/// Walk a registry, returning each registered item exactly once in reverse order in which they
/// were registered.
pub struct Walker<T> {
    next: *mut Registration<T>,
}

impl<T> Walker<T> {
    /// Construct a new walker from a registry.
    pub fn new(registry: &Registry<T>) -> Self {
        let next = registry.list.load(Ordering::Acquire);
        Self { next }
    }
}

impl<T: 'static> Iterator for Walker<T> {
    type Item = &'static T;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.next.is_null() {
            let registration: &Registration<T> = unsafe { &*self.next };
            self.next = registration.next.load(Ordering::Acquire);
            let item: &'static T = unsafe { &*registration.this.load(Ordering::Acquire) };
            Some(item)
        } else {
            None
        }
    }
}

/////////////////////////////////////////////// tests //////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    struct TestItem {
        value: u64,
        registration: Registration<TestItem>,
    }

    impl TestItem {
        const fn new(value: u64) -> Self {
            let registration = Registration::<TestItem>::new();
            Self {
                value,
                registration,
            }
        }

        fn ensure_registered(&'static self) {
            self.registration.ensure_registered(&REGISTRY, self);
        }
    }

    static ITEM_1: TestItem = TestItem::new(1);
    static ITEM_2: TestItem = TestItem::new(2);
    static ITEM_3: TestItem = TestItem::new(3);
    static ITEM_4: TestItem = TestItem::new(4);
    static ITEM_5: TestItem = TestItem::new(5);

    static REGISTRY: Registry<TestItem> = Registry::new();

    #[test]
    fn registration() {
        assert_eq!(0, REGISTRY.walk().count());
        ITEM_1.ensure_registered();
        assert_eq!(
            vec![1],
            REGISTRY.walk().map(|item| item.value).collect::<Vec<_>>()
        );
        ITEM_5.ensure_registered();
        assert_eq!(
            vec![5, 1],
            REGISTRY.walk().map(|item| item.value).collect::<Vec<_>>()
        );
        ITEM_2.ensure_registered();
        assert_eq!(
            vec![2, 5, 1],
            REGISTRY.walk().map(|item| item.value).collect::<Vec<_>>()
        );
        ITEM_4.ensure_registered();
        assert_eq!(
            vec![4, 2, 5, 1],
            REGISTRY.walk().map(|item| item.value).collect::<Vec<_>>()
        );
        ITEM_1.ensure_registered();
        ITEM_5.ensure_registered();
        ITEM_2.ensure_registered();
        ITEM_4.ensure_registered();
        ITEM_3.ensure_registered();
        assert_eq!(
            vec![3, 4, 2, 5, 1],
            REGISTRY.walk().map(|item| item.value).collect::<Vec<_>>()
        );
    }
}
