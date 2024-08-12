use std::collections::LinkedList;
use std::fmt::Debug;
use std::net::TcpStream;
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use boring::ssl::{SslConnector, SslFiletype, SslMethod};
use biometrics::{Collector, Counter};
use buffertk::{stack_pack, Unpacker};
use rpc_pb::sd::{Host, HostID};
use sync42::monitor::{Monitor, MonitorCore};
use sync42::spin_lock::SpinLock;
use sync42::state_hash_table::{Handle, StateHashTable};

use zerror_core::ErrorCore;

use super::channel::Channel;
use super::Resolver;

///////////////////////////////////////////// Constants ////////////////////////////////////////////

pub const MIN_CHANNELS: usize = 1;
pub const MAX_CHANNELS: usize = 5;

pub const MIN_USERSPACE_SEND_BUFFER: usize = 1 << 8;
pub const MAX_USERSPACE_SEND_BUFFER: usize = rpc_pb::MAX_BODY_SIZE * 2;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static GET_CHANNEL_ESTABLISHED: Counter = Counter::new("busyrpc.client.channel.established");
static GET_CHANNEL_WAIT: Counter = Counter::new("busyrpc.client.channel.wait");
static GET_CHANNEL_ERROR: Counter = Counter::new("busyrpc.client.channel.error");
static NEW_CHANNEL: Counter = Counter::new("busyrpc.client.channel.new");
static KILL_CHANNEL: Counter = Counter::new("busyrpc.client.channel.kill");
static SET_RESPONSE: Counter = Counter::new("busyrpc.client.set_response");
static SET_ERROR: Counter = Counter::new("busyrpc.client.set_error");
static DO_READ: Counter = Counter::new("busyrpc.client.read");
static READ_SPIN: Counter = Counter::new("busyrpc.client.read_spin");
static READ_TO_COMPLETION: Counter = Counter::new("busyrpc.client.read_to_completion");
static DO_WRITE: Counter = Counter::new("busyrpc.client.write");
static DO_POLL: Counter = Counter::new("busyrpc.client.poll");

pub fn register_biometrics(collector: &mut Collector) {
    collector.register_counter(&GET_CHANNEL_ESTABLISHED);
    collector.register_counter(&GET_CHANNEL_WAIT);
    collector.register_counter(&GET_CHANNEL_ERROR);
    collector.register_counter(&NEW_CHANNEL);
    collector.register_counter(&KILL_CHANNEL);
    collector.register_counter(&SET_RESPONSE);
    collector.register_counter(&SET_ERROR);
    collector.register_counter(&DO_READ);
    collector.register_counter(&READ_SPIN);
    collector.register_counter(&READ_TO_COMPLETION);
    collector.register_counter(&DO_WRITE);
    collector.register_counter(&DO_POLL);
}

/////////////////////////////////////////// ClientOptions //////////////////////////////////////////

/// BusyRPC client options.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "binaries", derive(arrrg_derive::CommandLine))]
pub struct ClientOptions {
    /// The number of channels to establish.
    #[cfg_attr(
        feature = "binaries",
        arrrg(optional, "Number of channels to establish.")
    )]
    pub channels: usize,
    /// SSL/TLS ca_file.
    #[cfg_attr(feature = "binaries", arrrg(required, "Path to the CA certificate."))]
    pub ca_file: String,
    /// Disable SSL verification.
    #[cfg_attr(feature = "binaries", arrrg(flag, "Do not verify SSL certificates."))]
    pub verify_none: bool,
    /// The user send-buffer size.
    #[cfg_attr(feature = "binaries", arrrg(optional, "Userspace send buffer size."))]
    pub user_send_buffer_size: usize,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            channels: 2,
            ca_file: "ca.crt".to_string(),
            verify_none: false,
            user_send_buffer_size: 65536,
        }
    }
}

impl ClientOptions {
    /// Set the number of channels to open in parallel.
    pub fn with_channels(mut self, channels: usize) -> Self {
        self.channels = channels.clamp(MIN_CHANNELS, MAX_CHANNELS);
        self
    }

    /// Set the user_send_buffer.
    pub fn with_user_send_buffer(mut self, user_send_buffer_size: usize) -> Self {
        self.user_send_buffer_size =
            user_send_buffer_size.clamp(MIN_USERSPACE_SEND_BUFFER, MAX_USERSPACE_SEND_BUFFER);
        self
    }
}

///////////////////////////////////////////// ShtState /////////////////////////////////////////////

mod sht_state {
    use super::*;

    /// The ShtState goes into the state hash table for rendezvous.
    #[derive(Debug)]
    pub struct ShtState {
        response: SpinLock<Option<Vec<u8>>>,
        error: SpinLock<Option<rpc_pb::Error>>,
    }

    impl ShtState {
        pub fn is_set(&self) -> bool {
            self.has_response() || self.has_error()
        }

        pub fn has_response(&self) -> bool {
            self.response.lock().is_some()
        }

        pub fn set_response(&self, response: Vec<u8>) {
            SET_RESPONSE.click();
            *self.response.lock() = Some(response);
        }

        pub fn get_response(&self) -> Option<Vec<u8>> {
            self.response.lock().take()
        }

        pub fn has_error(&self) -> bool {
            self.error.lock().is_some()
        }

        pub fn set_error(&self, err: rpc_pb::Error) {
            SET_ERROR.click();
            *self.error.lock() = Some(err);
        }

        pub fn get_error(&self) -> Option<rpc_pb::Error> {
            self.error.lock().take()
        }
    }

    impl Default for ShtState {
        fn default() -> Self {
            Self {
                response: SpinLock::new(None),
                error: SpinLock::new(None),
            }
        }
    }

    impl From<u64> for ShtState {
        fn from(_: u64) -> Self {
            Self::default()
        }
    }

    impl sync42::state_hash_table::Value for ShtState {
        fn finished(&self) -> bool {
            true
        }
    }
}

use sht_state::ShtState;

/////////////////////////////////////////// MonitorState ///////////////////////////////////////////

struct MonitorState<'a> {
    cv: &'a Condvar,
    requests: LinkedList<Vec<u8>>,
    sht: Handle<'a, u64, ShtState>,
    table: &'a StateHashTable<u64, ShtState>,
}

//////////////////////////////////////// ChannelCoordination ///////////////////////////////////////

#[derive(Default)]
struct ChannelCoordination {
    has_sender: bool,
    senders_waiting: usize,
    enqueued_requests: LinkedList<Vec<u8>>,
}

////////////////////////////////////// ChannelCriticalSection //////////////////////////////////////

struct ChannelCriticalSection {
    channel: Channel,
    close: bool,
}

impl ChannelCriticalSection {
    fn do_read<'a: 'b, 'b>(&'a mut self, ms: &'b mut MonitorState<'_>) -> bool {
        DO_READ.click();
        let mut buffers: Vec<Vec<u8>> = Vec::new();
        let buffers_mut = &mut buffers;
        let f = |buf| buffers_mut.push(buf);
        let would_block = match self.channel.do_recv_work(f) {
            Ok(would_block) => would_block,
            Err(err) => {
                ms.sht.set_error(err);
                return false;
            }
        };
        'buffersing: for buffer in buffers.into_iter() {
            let mut up = Unpacker::new(&buffer);
            let resp: rpc_pb::Response = match up.unpack() {
                Ok(resp) => resp,
                Err(err) => {
                    ms.sht.set_error(err.into());
                    continue 'buffersing;
                }
            };
            if let Some(handle) = ms.table.get_state(resp.seq_no) {
                handle.set_response(buffer);
            }
        }
        would_block
    }

    fn do_write<'a: 'b, 'b>(&'a mut self, ms: &'b mut MonitorState<'_>) -> bool {
        DO_WRITE.click();
        match self.channel.do_send_work() {
            Ok(write) => write,
            Err(err) => {
                ms.sht.set_error(err);
                true
            }
        }
    }
}

//////////////////////////////////////// ChannelMonitorCore ////////////////////////////////////////

#[derive(Debug, Default)]
struct ChannelMonitorCore;

impl<'c> MonitorCore<ChannelCoordination, ChannelCriticalSection, MonitorState<'c>>
    for ChannelMonitorCore
{
    fn acquire<'a: 'b, 'b>(
        &self,
        mut mtx: MutexGuard<'a, ChannelCoordination>,
        ms: &'b mut MonitorState<'_>,
    ) -> (bool, MutexGuard<'a, ChannelCoordination>) {
        mtx.enqueued_requests.append(&mut ms.requests);
        while mtx.has_sender && !ms.sht.is_set() {
            mtx.senders_waiting += 1;
            mtx = ms.cv.wait(mtx).unwrap();
            mtx.senders_waiting -= 1;
        }
        if ms.sht.is_set() {
            if mtx.senders_waiting > 0 {
                ms.cv.notify_one();
            }
            (false, mtx)
        } else {
            ms.requests.append(&mut mtx.enqueued_requests);
            mtx.has_sender = true;
            (true, mtx)
        }
    }

    fn release<'a: 'b, 'b>(
        &self,
        mut mtx: MutexGuard<'a, ChannelCoordination>,
        ms: &'b mut MonitorState<'_>,
    ) -> MutexGuard<'a, ChannelCoordination> {
        mtx.has_sender = false;
        if mtx.senders_waiting > 0 {
            ms.cv.notify_one();
        }
        mtx
    }

    fn critical_section<'a: 'b, 'b>(
        &self,
        crit: &'a mut ChannelCriticalSection,
        ms: &'b mut MonitorState<'_>,
    ) {
        let mut requests = LinkedList::default();
        std::mem::swap(&mut requests, &mut ms.requests);
        for req in requests.into_iter() {
            if let Err(err) = crit.channel.send(&req) {
                ms.sht.set_error(err);
                return;
            }
        }
        if crit.close {
            ms.sht.set_error(rpc_pb::Error::TransportFailure {
                core: ErrorCore::default(),
                what: "transport closed".to_owned(),
            });
            return;
        }
        let mut events = libc::POLLIN | libc::POLLOUT | libc::POLLERR | libc::POLLHUP;
        while !ms.sht.is_set() {
            let mut pfd = libc::pollfd {
                fd: crit.channel.as_raw_fd(),
                events,
                revents: 0,
            };
            unsafe {
                DO_POLL.click();
                if libc::poll(&mut pfd, 1, -1) < 0 {
                    ms.sht.set_error(std::io::Error::last_os_error().into());
                    return;
                }
            }
            if pfd.revents & libc::POLLIN != 0 {
                while !crit.do_read(ms) && !ms.sht.has_error() {
                    READ_SPIN.click();
                }
                READ_TO_COMPLETION.click();
            }
            if pfd.revents & libc::POLLOUT != 0 && !crit.do_write(ms) {
                events &= !libc::POLLOUT;
            }
            if pfd.revents & (libc::POLLERR | libc::POLLHUP) != 0 {
                crit.close = true;
            }
        }
    }
}

///////////////////////////////////////// MonitoredChannel /////////////////////////////////////////

struct MonitoredChannel<'a: 'b, 'b> {
    monitor:
        Monitor<ChannelCoordination, ChannelCriticalSection, MonitorState<'b>, ChannelMonitorCore>,
    available: Condvar,
    _a: std::marker::PhantomData<&'a ()>,
}

////////////////////////////////////////////// HostKey /////////////////////////////////////////////

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
struct HostKey {
    host_id: HostID,
}

impl From<HostID> for HostKey {
    fn from(host_id: HostID) -> Self {
        Self { host_id }
    }
}

impl sync42::state_hash_table::Key for HostKey {}

//////////////////////////////////////// EstablishmentState ////////////////////////////////////////

#[derive(Default)]
struct EstablishmentState<'a, 'b> {
    wait: Condvar,
    done: Mutex<(bool, bool)>,
    value: Mutex<Option<Arc<MonitoredChannel<'a, 'b>>>>,
}

impl<'a, 'b> From<HostKey> for EstablishmentState<'a, 'b> {
    fn from(_: HostKey) -> Self {
        Self::default()
    }
}

impl<'a, 'b> sync42::state_hash_table::Value for EstablishmentState<'a, 'b> {
    fn finished(&self) -> bool {
        true
    }
}

/////////////////////////////////////////// ChannelHandle //////////////////////////////////////////

struct ChannelHandle<'a, 'b, 'c, R: Resolver> {
    channel: Arc<MonitoredChannel<'a, 'b>>,
    manager: &'c ChannelManager<'a, 'b, R>,
    index: usize,
}

impl<'a, 'b, 'c, R: Resolver> ChannelHandle<'a, 'b, 'c, R> {
    fn kill(&self) {
        let mut channels = self.manager.channels.lock().unwrap();
        if let Some((_, channel)) = &channels[self.index] {
            if Arc::as_ptr(channel) == Arc::as_ptr(&self.channel) {
                channels[self.index] = None;
            }
        }
    }
}

/////////////////////////////////////// ChannelManangerTrait ///////////////////////////////////////

trait ChannelManagerTrait<R: Resolver> {
    type ChannelHandle<'c>
    where
        Self: 'c;
    type MonitoredChannel;

    fn new(options: ClientOptions, resolver: R) -> Self;
    fn get_channel(&self) -> Result<Self::ChannelHandle<'_>, rpc_pb::Error>;
    fn establish_channel(&self, host: &Host) -> Result<Arc<Self::MonitoredChannel>, rpc_pb::Error>;
    fn new_channel(&self, host: &Host) -> Result<Arc<Self::MonitoredChannel>, rpc_pb::Error>;
    fn register_channel(&self, host: &Host, chan: Arc<Self::MonitoredChannel>);
}

////////////////////////////////////////// ChannelManager //////////////////////////////////////////

struct ChannelManager<'a, 'b, R: Resolver> {
    options: ClientOptions,
    resolver: Mutex<R>,
    // NOTE(rescrv): I like seeing the type, not hiding it.
    #[allow(clippy::type_complexity)]
    channels: Mutex<Vec<Option<(HostID, Arc<MonitoredChannel<'a, 'b>>)>>>,
    connecting: StateHashTable<HostKey, EstablishmentState<'a, 'b>>,
}

impl<'a, 'b, R: Resolver> ChannelManagerTrait<R> for ChannelManager<'a, 'b, R> {
    type ChannelHandle<'c> = ChannelHandle<'a, 'b, 'c, R> where Self: 'c;
    type MonitoredChannel = MonitoredChannel<'a, 'b>;

    fn new(options: ClientOptions, resolver: R) -> Self {
        Self {
            options,
            resolver: Mutex::new(resolver),
            channels: Mutex::default(),
            connecting: StateHashTable::new(),
        }
    }

    fn get_channel(&self) -> Result<Self::ChannelHandle<'_>, rpc_pb::Error> {
        let resolved = { self.resolver.lock().unwrap().resolve()? };
        loop {
            {
                let channels = self.channels.lock().unwrap();
                for (index, channel) in channels.iter().enumerate() {
                    if let Some((host_id, channel)) = channel {
                        if *host_id == resolved.host_id() {
                            let channel = Arc::clone(channel);
                            return Ok(ChannelHandle {
                                channel,
                                manager: self,
                                index,
                            });
                        }
                    }
                }
            }
            self.establish_channel(&resolved)?;
        }
    }

    fn establish_channel(&self, host: &Host) -> Result<Arc<Self::MonitoredChannel>, rpc_pb::Error> {
        let establish = self
            .connecting
            .get_or_create_state(HostKey::from(host.host_id()));
        let mut done = establish.done.lock().unwrap();
        while done.0 && !done.1 {
            GET_CHANNEL_WAIT.click();
            done = establish.wait.wait(done).unwrap();
        }
        if !done.0 {
            done.0 = true;
            let channel = match self.new_channel(host) {
                Ok(channel) => channel,
                Err(err) => {
                    GET_CHANNEL_ERROR.click();
                    done.0 = false;
                    establish.wait.notify_one();
                    return Err(err);
                }
            };
            *establish.value.lock().unwrap() = Some(Arc::clone(&channel));
            self.register_channel(host, Arc::clone(&channel));
            done.1 = true;
            establish.wait.notify_all();
            return Ok(channel);
        }
        // SAFETY(rescrv): This unwrap should never fail because we set it above prior to the
        // notify_all().
        let ptr = Arc::clone(establish.value.lock().unwrap().as_ref().unwrap());
        Ok(ptr)
    }

    fn new_channel(&self, host: &Host) -> Result<Arc<Self::MonitoredChannel>, rpc_pb::Error> {
        let mut builder = SslConnector::builder(SslMethod::tls()).map_err(|err| {
            rpc_pb::Error::EncryptionMisconfiguration {
                core: ErrorCore::default(),
                what: format!("could not build connector builder: {}", err),
            }
        })?;
        builder.set_ca_file(&self.options.ca_file).map_err(|err| {
            rpc_pb::Error::EncryptionMisconfiguration {
                core: ErrorCore::default(),
                what: format!("invalid CA file: {}", err),
            }
        })?;
        if self.options.verify_none {
            builder.set_verify(boring::ssl::SslVerifyMode::NONE);
        }
        let connector = builder.build();
        let stream = TcpStream::connect(host.connect())?;
        let stream = connector.connect(host.hostname_or_ip(), stream).map_err(|err| {
            rpc_pb::Error::TransportFailure {
                core: ErrorCore::default(),
                what: format!("{}", err),
            }
        })?;
        let channel = Channel::new(stream, self.options.user_send_buffer_size)?;
        let monitored_channel = MonitoredChannel {
            monitor: Monitor::new(
                ChannelMonitorCore,
                ChannelCoordination::default(),
                ChannelCriticalSection {
                    channel,
                    close: false,
                },
            ),
            available: Condvar::new(),
            _a: std::marker::PhantomData,
        };
        NEW_CHANNEL.click();
        Ok(Arc::new(monitored_channel))
    }

    fn register_channel(&self, host: &Host, chan: Arc<Self::MonitoredChannel>) {
        let mut channels = self.channels.lock().unwrap();
        for channel in channels.iter_mut() {
            if channel.is_none() {
                *channel = Some((host.host_id(), Arc::clone(&chan)));
                return;
            }
        }
        channels.push(Some((host.host_id(), Arc::clone(&chan))));
    }
}

////////////////////////////////////////////// Client //////////////////////////////////////////////

pub struct Client<'a: 'b, 'b, R: Resolver + Send + Sync> {
    sequencer: AtomicU64,
    concurrent_ops: StateHashTable<u64, ShtState>,
    channels: ChannelManager<'a, 'b, R>,
}

impl<'a, 'b, R: Resolver + Send + Sync + 'static> Client<'a, 'b, R>
where
    'a: 'b,
{
    // NOTE(rescrv): allow new_ret_no_self because we want to return something that hides the
    // lifetimes on the client, making for beautiful code elsewhere.  Threading around two
    // lifetimes proved to be too much, even for me.
    #[allow(clippy::new_ret_no_self)]
    fn new(options: ClientOptions, resolver: R) -> Arc<dyn rpc_pb::Client + Send + Sync + 'b> {
        Arc::new(Self {
            sequencer: AtomicU64::new(1),
            concurrent_ops: StateHashTable::new(),
            channels: ChannelManager::new(options, resolver),
        })
    }

    fn get_any_channel(&self) -> Result<ChannelHandle<'a, 'b, '_, R>, rpc_pb::Error> {
        self.channels.get_channel()
    }
}

impl<R: Resolver + Send + Sync + 'static> rpc_pb::Client for Client<'_, '_, R> {
    fn call(
        &self,
        ctx: &rpc_pb::Context,
        service: &str,
        method: &str,
        body: &[u8],
    ) -> rpc_pb::Status {
        if body.len() > rpc_pb::MAX_REQUEST_SIZE {
            return Err(rpc_pb::Error::RequestTooLarge {
                core: ErrorCore::default(),
                size: body.len() as u64,
            });
        }
        let caller = ctx.clients().to_vec();
        let req = rpc_pb::Request {
            service,
            method,
            seq_no: self.sequencer.fetch_add(1, Ordering::Relaxed),
            body,
            caller,
            trace: ctx.trace_id(),
        };
        let seq_no = req.seq_no;
        let handle = self.concurrent_ops.get_or_create_state(seq_no);
        let req_buf = stack_pack(req).to_vec();
        let chandle = self.get_any_channel()?;
        let mut requests = LinkedList::default();
        requests.push_back(req_buf);
        let mut ms = MonitorState {
            cv: &chandle.channel.available,
            requests,
            sht: handle,
            table: &self.concurrent_ops,
        };
        chandle.channel.monitor.do_it(&mut ms);
        if let Some(err) = ms.sht.get_error() {
            chandle.kill();
            return Err(err);
        }
        let status = if let Some(resp_buf) = ms.sht.get_response() {
            let mut up = Unpacker::new(&resp_buf);
            let resp: rpc_pb::Response = match up.unpack() {
                Ok(resp) => resp,
                Err(err) => {
                    chandle.kill();
                    return Err(err.into());
                }
            };
            if let Some(rpc_error) = resp.rpc_error {
                let mut up = Unpacker::new(rpc_error);
                let rpc_error: rpc_pb::Error = match up.unpack() {
                    Ok(rpc_error) => rpc_error,
                    Err(unpack_error) => unpack_error.into(),
                };
                Err(rpc_error)
            } else if let Some(service_error) = resp.service_error {
                Ok(Err(service_error.to_vec()))
            } else if let Some(body) = resp.body {
                Ok(Ok(body.to_vec()))
            } else {
                chandle.kill();
                Err(rpc_pb::Error::LogicError {
                    core: ErrorCore::default(),
                    what: "missing rpc_error, service_error, and body; at least one should be set"
                        .to_owned(),
                })
            }
        } else {
            chandle.kill();
            Err(rpc_pb::Error::LogicError {
                core: ErrorCore::default(),
                what: "dropped value in response".to_owned(),
            })
        };
        status
    }
}

/// Create a new client from the options and resolver.
pub fn new_client<R: Resolver + Send + Sync + 'static>(
    options: ClientOptions,
    resolver: R,
) -> Arc<dyn rpc_pb::Client + Send + Sync> {
    Client::new(options, resolver)
}
