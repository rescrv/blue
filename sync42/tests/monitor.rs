use std::sync::{Condvar, MutexGuard};

use sync42::monitor::{Monitor, MonitorCore};

struct TestState<'a> {
    cnd: &'a Condvar,
}

#[derive(Default)]
struct TestCoordination {
    someone_has_it: bool,
    threads_waiting: usize,
}

#[derive(Default)]
struct TestCriticalSection {
    count: u64,
}

#[derive(Default)]
struct TestMonitor;

impl MonitorCore<TestCoordination, TestCriticalSection, TestState<'_>> for TestMonitor {
    fn acquire<'a: 'b, 'b>(
        &self,
        mut guard: MutexGuard<'a, TestCoordination>,
        t: &'b mut TestState,
    ) -> (bool, MutexGuard<'a, TestCoordination>) {
        while guard.someone_has_it {
            guard.threads_waiting += 1;
            guard = t.cnd.wait(guard).unwrap();
            guard.threads_waiting -= 1;
        }
        guard.someone_has_it = true;
        (true, guard)
    }

    fn release<'a: 'b, 'b>(
        &self,
        mut guard: MutexGuard<'a, TestCoordination>,
        t: &'b mut TestState,
    ) -> MutexGuard<'a, TestCoordination> {
        guard.someone_has_it = false;
        if guard.threads_waiting > 0 {
            t.cnd.notify_one();
        }
        guard
    }

    fn critical_section<'a: 'b, 'b>(
        &self,
        crit: &'a mut TestCriticalSection,
        _t: &'b mut TestState<'_>,
    ) {
        crit.count += 1;
    }
}

#[test]
fn monitor_outside_struct() {
    let core = TestMonitor;
    let coordination = TestCoordination::default();
    let critical_section = TestCriticalSection::default();
    let monitor = Monitor::new(core, coordination, critical_section);
    let cnd = Condvar::new();
    let mut state = TestState { cnd: &cnd };
    monitor.do_it(&mut state);
}

struct TestWithMonitor<'a> {
    monitor: Monitor<TestCoordination, TestCriticalSection, TestState<'a>, TestMonitor>,
}

#[test]
fn monitor_inside_struct() {
    let core = TestMonitor;
    let coordination = TestCoordination::default();
    let critical_section = TestCriticalSection::default();
    let monitor = Monitor::new(core, coordination, critical_section);
    let twm = TestWithMonitor { monitor };
    let cnd = Condvar::new();
    let mut state = TestState { cnd: &cnd };
    twm.monitor.do_it(&mut state);
}
