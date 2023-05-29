use std::net::TcpStream;
use std::sync::{Arc, Mutex};

use buffertk::{stack_pack, Buffer, Unpackable};

use biometrics::{Counter, Collector};

use zerror_core::ErrorCore;

use rpc_pb::{Context, Error, Status};

use sync42::wait_list::WaitList;

use boring::ssl::SslStream;

use rivulet::{RecvChannel, SendChannel};

use super::Resolver;

//////////////////////////////////////////// biometrics ////////////////////////////////////////////

static REQUEST_BEGIN: Counter = Counter::new("busybee.client.request_begin");
static REQUEST_SEND_FAILURE: Counter = Counter::new("busybee.client.request_sent");
static REQUEST_SENT: Counter = Counter::new("busybee.client.request_sent");
static REQUEST_AT_HEAD: Counter = Counter::new("busybee.client.request_at_head");
static RESPONSE_RECEIVED: Counter = Counter::new("busybee.client.response_received");

pub fn register_biometrics(collector: &mut Collector) {
    collector.register_counter(&REQUEST_BEGIN);
    collector.register_counter(&REQUEST_SEND_FAILURE);
    collector.register_counter(&REQUEST_SENT);
    collector.register_counter(&REQUEST_AT_HEAD);
}

////////////////////////////////////////////// Channel /////////////////////////////////////////////

#[derive(Debug)]
struct Channel {
    recv: Mutex<RecvChannel>,
    send: Mutex<SendChannel>,
    poison: Mutex<Option<Error>>,
}

impl Channel {
    fn new(stream: SslStream<TcpStream>) -> Result<Channel, Error> {
        let (r, s) = rivulet::from_stream(stream)?;
        Ok(Channel::from_parts(r, s))
    }

    fn from_parts(recv: RecvChannel, send: SendChannel) -> Channel {
        Channel {
            recv: Mutex::new(recv),
            send: Mutex::new(send),
            poison: Mutex::new(None),
        }
    }

    fn call(
        &self,
        ctx: &Context,
        wait_list: &WaitList<Option<Status>>,
        server: &str,
        method: &str,
        body: &[u8],
    ) -> Status {
        let caller = ctx.clients().to_vec();
        let mut waiter = wait_list.link(None);
        REQUEST_BEGIN.click();
        let req = rpc_pb::Request {
            server,
            method,
            seq_no: waiter.index(),
            body,
            caller,
            trace: ctx.trace_id(),
        };
        let req_buf = stack_pack(req).to_buffer();
        if let Err(err) = self.send.lock().unwrap().send(req_buf.as_bytes()) {
            REQUEST_SEND_FAILURE.click();
            wait_list.unlink(waiter);
            return Err(err);
        }
        REQUEST_SENT.click();
        let mut recv = self.recv.lock().unwrap();
        // NOTE(rescrv): We do not check for poisoning here.  We let this drain the head one node
        // at a time, and the head will check for poisoning as its first or second act as head.
        while waiter.load().is_none() && !waiter.is_head() {
            recv = waiter.naked_wait(recv);
        }
        REQUEST_AT_HEAD.click();
        'receiving:
        loop {
            if let Some(Some(resp)) = waiter.load() {
                wait_list.unlink(waiter);
                // TODO(rescrv): Conditional notify head.
                wait_list.notify_head();
                return resp;
            }
            // TODO(rescrv): Check for poisoning.
            if !waiter.is_head() {
                recv = waiter.naked_wait(recv);
                // Take it from the top to check if we have a value.
                continue 'receiving;
            }
            match recv.recv() {
                Ok(resp_buf) => {
                    let resp = match rpc_pb::Response::unpack(resp_buf.as_bytes()) {
                        Ok((resp, buf)) => {
                            if !buf.is_empty() {
                                rpc_pb::UNUSED_BUFFER.click()
                            }
                            resp
                        },
                        Err(err) => {
                            self.poison(err.into())?;
                            todo!();
                        },
                    };
                    RESPONSE_RECEIVED.click();
                    let resper = waiter.get_waiter(resp.seq_no);
                    if let Some(mut resper) = resper {
                        if let Some(rpc_error) = resp.rpc_error {
                            let rpc_error = <rpc_pb::Error as Unpackable>::unpack(rpc_error)?.0;
                            resper.store(Some(Err(rpc_error)));
                        } else if let Some(service_err) = resp.service_error {
                            resper.store(Some(Ok(Err(Buffer::from(service_err)))));
                        } else if let Some(body) = resp.body {
                            resper.store(Some(Ok(Ok(Buffer::from(body)))));
                        } else {
                            self.poison(Error::TransportFailure {
                                core: ErrorCore::default(),
                                what: "none of rpc_err, service_err, body were set".to_string(),
                            })?;
                        }
                    } else {
                        self.poison(Error::TransportFailure {
                            core: ErrorCore::default(),
                            what: format!(
                                "seq_no={} does not correspond to a receiver",
                                resp.seq_no
                            ),
                        })?;
                    }
                },
                Err(err) => {
                    self.poison(err)?;
                },
            }
        }
    }

    fn poison(&self, err: Error) -> Result<(), Error> {
        let mut guard = self.poison.lock().unwrap();
        if guard.is_none() {
            *guard = Some(err.clone())
        }
        Err(err)
    }
}

/////////////////////////////////////////// ClientOptions //////////////////////////////////////////

#[derive(Debug)]
pub struct ClientOptions {
    channels: usize,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            channels: 2,
        }
    }
}

///////////////////////////////////////////// Internals ////////////////////////////////////////////

#[derive(Debug, Default)]
struct Internals {
    channels: Vec<Arc<Channel>>,
    round_robin: usize,
}

////////////////////////////////////////////// Client //////////////////////////////////////////////

pub struct Client {
    options: ClientOptions,
    resolver: Box<dyn Resolver>,
    wait_list: WaitList<Option<Status>>,
    internals: Mutex<Internals>,
}

impl Client {
    pub fn new(options: ClientOptions, resolver: Box<dyn Resolver>) -> Self {
        Self {
            options,
            resolver,
            wait_list: WaitList::default(),
            internals: Mutex::new(Internals::default()),
        }
    }

    fn get_channel(&self, ctx: &Context) -> Result<Arc<Channel>, Error> {
        let mut internals = self.internals.lock().unwrap();
        let chan = if internals.channels.len() < self.options.channels {
            let chan = Channel::new(self.resolver.lookup(ctx)?)?;
            let chan = Arc::new(chan);
            internals.channels.push(Arc::clone(&chan));
            chan
        } else {
            let round_robin = (internals.round_robin + 1) % internals.channels.len();
            internals.round_robin = round_robin;
            Arc::clone(&internals.channels[round_robin])
        };
        Ok(chan)
    }
}

impl rpc_pb::Client for Client {
    fn call(
        &self,
        ctx: &Context,
        server: &str,
        method: &str,
        body: &[u8],
    ) -> Status {
        if body.len() > rpc_pb::MAX_REQUEST_SIZE {
            return Err(Error::RequestTooLarge {
                core: ErrorCore::default(),
                size: body.len() as u64,
            });
        }
        let chan = self.get_channel(ctx)?;
        chan.call(ctx, &self.wait_list, server, method, body)
    }
}
