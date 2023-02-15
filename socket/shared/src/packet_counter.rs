use std::borrow::Cow;

use atomic_counter::{RelaxedCounter, AtomicCounter};
use log::*;

#[derive(Debug)]
pub struct PacketCounter {
	context: Cow<'static, String>,
	pub recv: RelaxedCounter,
	pub sent: RelaxedCounter,
}
impl PacketCounter {
	pub fn new(context: impl Into<String>) -> Self {
		Self {
			context: Cow::Owned(context.into()),
			recv: Default::default(),
			sent: Default::default(),
		}
	}
	pub fn inc_recv(&self) { self.recv.inc(); }
	pub fn inc_sent(&self) { self.sent.inc(); }
}

impl Drop for PacketCounter {
	fn drop(&mut self) {
		info!("{:?}", self);
	}
}