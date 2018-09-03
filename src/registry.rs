// Copyright (c) 2013-2016 Sandstorm Development Group, Inc. and contributors
// Licensed under the MIT License:
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
// THE SOFTWARE.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use capnp_rpc::{rpc_twoparty_capnp, twoparty, RpcSystem};
use pubsub_capnp::{publisher, registration, registry, resource_locator, subscription};

use capnp::capability::Promise;
use capnp::message::TypedReader;
use capnp::serialize::OwnedSegments;
use capnp::Error;

use futures::{Future, Stream, MapErr};

use tokio_core::reactor;
use tokio_io::AsyncRead;

use regex::Regex;

#[derive(PartialEq, Eq)]
enum ResourceLocator {
    HostPair { host: String, port: u32 },
    URL(String),
}

#[derive(PartialEq, Eq)]
struct PublisherHandle {
    id: u64,
    locator: ResourceLocator,
}

struct PublisherMap {
    publishers: HashMap<String, Vec<PublisherHandle>>,
}

impl PublisherMap {
    fn new() -> PublisherMap {
        PublisherMap {
            publishers: HashMap::new(),
        }
    }
}

struct RegistrationImpl {
    id: u64,
    topic: String,
    publishers: Rc<RefCell<PublisherMap>>,
}

impl RegistrationImpl {
    fn new(id: u64, topic: String, publishers: Rc<RefCell<PublisherMap>>) -> RegistrationImpl {
        RegistrationImpl {
            id,
            topic,
            publishers,
        }
    }
}

impl Drop for RegistrationImpl {
    fn drop(&mut self) {
        println!("Registration dropped");
        let len = self
            .publishers
            .borrow()
            .publishers
            .get(&self.topic)
            .unwrap()
            .len();
        if len == 1 {
            self.publishers.borrow_mut().publishers.remove(&self.topic);
        } else if len > 1 {
            let mut pub_map = self.publishers.borrow_mut();
            let pub_list = pub_map.publishers.get_mut(&self.topic).unwrap();

            let index = pub_list.iter().position(|x| x.id == self.id).unwrap();
            pub_list.remove(index);
        } else {
            panic!("self.publishers.borrow().publishers.get(&self.topic).unwrap().len() zero");
        }
    }
}

impl registration::Server for RegistrationImpl {}

struct RegistryImpl {
    next_id: u64,
    publishers: Rc<RefCell<PublisherMap>>,
}

impl RegistryImpl {
    pub fn new() -> (RegistryImpl, Rc<RefCell<PublisherMap>>) {
        let publishers = Rc::new(RefCell::new(PublisherMap::new()));
        (
            RegistryImpl {
                next_id: 0,
                publishers: publishers.clone(),
            },
            publishers.clone(),
        )
    }
}

impl registry::Server for RegistryImpl {
    fn register(
        &mut self,
        params: registry::RegisterParams,
        mut results: registry::RegisterResults,
    ) -> Promise<(), ::capnp::Error> {
        println!("register");

        let re = Regex::new(r"(/[\w\\.]+)+\z").unwrap();
        let topic = pry!(pry!(params.get()).get_topic());
        let locator = pry!(pry!(params.get()).get_publisher());
        if !re.is_match(topic) {
            println!("subscription failed");
            return Promise::err(::capnp::Error::failed("topic does not exist".to_string()));
        }

        // let mut pub_map = ;

        let mut pub_map = self.publishers.borrow_mut();

        let p_vec = pub_map.publishers.entry(topic.to_string()).or_insert(Vec::new());

        p_vec.push(PublisherHandle {
            id: self.next_id,
            locator: match pry!(locator.which()) {
                resource_locator::HostPair(hp) => ResourceLocator::HostPair {
                    host: hp.get_host().unwrap().to_string(),
                    port: hp.get_port(),
                },
                resource_locator::Url(url) => ResourceLocator::URL(url.unwrap().to_string()),
            },
        });
        
        results.get().set_handle(
            registration::ToClient::new(RegistrationImpl::new(
                self.next_id,
                topic.to_string(),
                self.publishers.clone(),
            )).from_server::<::capnp_rpc::Server>(),
        );

        self.next_id += 1;
        Promise::ok(())
    }
}

pub fn main() {
    use std::net::ToSocketAddrs;
    let args: Vec<String> = ::std::env::args().collect();
    if args.len() != 3 {
        println!("usage: {} registry HOST:PORT", args[0]);
        return;
    }

    let mut core = reactor::Core::new().unwrap();
    let handle = core.handle();

    let addr = args[2]
        .to_socket_addrs()
        .unwrap()
        .next()
        .expect("could not parse address");
    let socket = ::tokio_core::net::TcpListener::bind(&addr, &handle).unwrap();

    let (registry_impl, publishers) = RegistryImpl::new();

    let registry = registry::ToClient::new(registry_impl).from_server::<::capnp_rpc::Server>();

    let handle1 = handle.clone();
    let done = socket.incoming().for_each(move |(socket, _addr)| {
        try!(socket.set_nodelay(true));
        let (reader, writer) = socket.split();
        let handle = handle1.clone();

        let network =
            twoparty::VatNetwork::new(reader, writer,
                                      rpc_twoparty_capnp::Side::Server, Default::default());

        let rpc_system = RpcSystem::new(Box::new(network), Some(registry.clone().client));

        handle.spawn(rpc_system.map_err(|_| ()));
        Ok(())
    }).map_err(|e| e.into());

    let infinite = ::futures::stream::iter_ok::<_, Error>(::std::iter::repeat(()));
    let main_loop = infinite.fold((handle, publishers), move |(handle, publishers), ()| -> Promise<(::tokio_core::reactor::Handle, Rc<RefCell<PublisherMap>>), Error> {
        {
            
            
        }

        let timeout = pry!(::tokio_core::reactor::Timeout::new(::std::time::Duration::from_millis(500), &handle));
        let timeout = timeout.and_then(move |()| Ok((handle, publishers))).map_err(|e| e.into());
        Promise::from_future(timeout)
    });
    //core.run(done).unwrap();
    core.run(main_loop.join(done)).unwrap();
}
