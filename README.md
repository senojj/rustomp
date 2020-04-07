# rustomp

**Note: this is a work in progress**

rustomp is a lightweight Rust library for working with the STOMP messaging protocol.

### Current State
The `frame` module provides data structures and methods for reading and writing STOMP frames.

The unique feature of this STOMP library is the delay of reading the message body. This means that
when calling `Frame::read_from`, a `Frame` is returned, but the message body is left unread on the
input stream until `Frame::body` is read, `Frame::body::close` is called, or the `Frame` is dropped.

Choosing to not automatically read the message body into memory is a choice made with a concern for
effective use of memory, and security. In the event that a large message is received from either a
client or server, the data can be processed in a streaming manner, without the library imposing an 
arbitrary content length limit. **Note: this means that a sequential frame cannot be read until the 
current `Frame`'s `body` has been read or closed, or the current `Frame` has been dropped.** 
Therefore, it is wise to process the message body as quickly as possible, so that reading from the 
input stream may resume.

### Future State
Eventually, this library may contain full, multithreaded client and server implementations.