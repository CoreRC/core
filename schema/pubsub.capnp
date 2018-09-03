@0xce579c3e9bb684bd;

using Cxx = import "/capnp/c++.capnp";
$Cxx.namespace("corerc");

interface Subscription {}

interface Publisher(T) {
    # A source of messages of type T.

    subscribe @0 (topic: Text, subscriber: Subscriber(T)) -> (subscription: Subscription);
    # Registers `subscriber` to receive published messages. Dropping the returned `subscription`
    # signals to the `Publisher` that the subscriber is no longer interested in receiving messages.
}

interface Subscriber(T) {
    pushMessage @0 (message: T) -> ();
    # Sends a message from a publisher to the subscriber. To help with flow control, the subscriber should not
    # return from this method until it is ready to process the next message.
}

struct ResourceLocator {
    # IP address and port ...
    union {
        hostPair :group {
            host @0: Text;
            port @1: UInt32;
        }
        url @2: Text;
    }
}

interface Registration {}

interface Registry {
     register @0 (publisher :ResourceLocator, topic :Text) -> (handle: Registration);
     getPublishersForTopic @1 (topic :Text) -> (publishers :List(ResourceLocator));
}