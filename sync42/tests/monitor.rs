use std::sync::{Arc, Condvar, MutexGuard};

use sync42::monitor::{Coordination, CriticalSection, Monitor};

struct TestState<'a> {
    cnd: &'a Condvar,
}

#[derive(Default)]
struct TestCoordination {
    someone_has_it: bool,
    threads_waiting: usize,
}

impl Coordination<TestState<'_>> for TestCoordination {
    fn acquire<'a: 'b, 'b>(
        mut guard: MutexGuard<'a, Self>,
        t: &'b mut TestState<'_>,
    ) -> (bool, MutexGuard<'a, Self>) {
        while guard.someone_has_it {
            guard.threads_waiting += 1;
            guard = t.cnd.wait(guard).unwrap();
            guard.threads_waiting -= 1;
        }
        guard.someone_has_it = true;
        (true, guard)
    }

    fn release<'a: 'b, 'b>(
        mut guard: MutexGuard<'a, Self>,
        t: &'b mut TestState<'_>,
    ) -> MutexGuard<'a, Self> {
        guard.someone_has_it = false;
        if guard.threads_waiting > 0 {
            t.cnd.notify_one();
        }
        guard
    }
}

#[derive(Default)]
struct TestCriticalSection {
    count: u64,
}

impl CriticalSection<TestState<'_>> for TestCriticalSection {
    fn critical_section<'a: 'b, 'b>(&'a mut self, _t: &'b mut TestState<'_>) {
        self.count += 1;
    }
}

#[test]
fn monitor_outside_struct() {
    let coordination = TestCoordination::default();
    let critical_section = TestCriticalSection::default();
    let monitor = Monitor::<TestState, TestCoordination, TestCriticalSection>::new(
        coordination,
        critical_section,
    );
    let cnd = Condvar::new();
    let mut state = TestState { cnd: &cnd };
    monitor.do_it(&mut state);
}

struct TestWithMonitor<'a: 'b, 'b> {
    monitor: Monitor<TestState<'b>, TestCoordination, TestCriticalSection>,
    _a: std::marker::PhantomData<&'a ()>,
}

#[test]
fn monitor_inside_struct() {
    let coordination = TestCoordination::default();
    let critical_section = TestCriticalSection::default();
    let monitor = Monitor::<TestState, TestCoordination, TestCriticalSection>::new(
        coordination,
        critical_section,
    );
    let twm = TestWithMonitor {
        monitor,
        _a: std::marker::PhantomData,
    };
    let cnd = Condvar::new();
    let mut state = TestState { cnd: &cnd };
    twm.monitor.do_it(&mut state);
}

struct TestWithMonitorReturn<'a: 'b, 'b> {
    monitor: Arc<Monitor<TestState<'b>, TestCoordination, TestCriticalSection>>,
    _a: std::marker::PhantomData<&'a ()>,
}

impl<'a, 'b> TestWithMonitorReturn<'a, 'b> {
    fn get_monitor(&self) -> Arc<Monitor<TestState<'b>, TestCoordination, TestCriticalSection>> {
        Arc::clone(&self.monitor)
    }
}

#[test]
fn monitor_returns_arc() {
    let coordination = TestCoordination::default();
    let critical_section = TestCriticalSection::default();
    let monitor = Monitor::<TestState, TestCoordination, TestCriticalSection>::new(
        coordination,
        critical_section,
    );
    let twm = TestWithMonitorReturn {
        monitor: Arc::new(monitor),
        _a: std::marker::PhantomData,
    };
    let cnd = Condvar::new();
    let mut state = TestState { cnd: &cnd };
    twm.get_monitor().do_it(&mut state);
}
