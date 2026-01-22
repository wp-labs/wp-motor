//! TCP worker module: listener loop dispatching accepted sockets to sources.

mod listener_loop;

pub use listener_loop::{ConnectionRegistration, TcpListenerLoop};
