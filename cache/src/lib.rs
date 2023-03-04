////////////////////////////////////////// AdmissionPolicy /////////////////////////////////////////

/// An [AdmissionPolicy] is a gatekeeping force at the entry to the cache.  A good admission policy
/// will only replace objects in the cache when the caching outcome is favorable.
pub trait AdmissionPolicy {
    /// `t` gets admitted to the cache.  Update accordingly.
    fn admit<T: std::hash::Hash>(&self, t: &T);
    /// Return true if the `candidate` should replace the `victim`.
    fn should_replace<T: std::hash::Hash>(&self, victim: &T, candidate: &T) -> bool;
}
