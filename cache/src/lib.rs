////////////////////////////////////////// AdmissionPolicy /////////////////////////////////////////

pub trait AdmissionPolicy {
    fn admit<T: std::hash::Hash>(&self, t: &T);
    fn should_replace<T: std::hash::Hash>(&self, victim: &T, candidate: &T) -> bool;
}
