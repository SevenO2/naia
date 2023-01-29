
// use tokio::sync::mpsc;

// #[derive(Debug)]
// pub enum Signal {
// 	Offer(String),
// 	Answer(String),
// 	Candidate(String),
// }

// #[derive(Debug, Display)]
// pub enum SignalError {
// 	BufferFull,
// 	ChannelClosed,
// }
// impl std::error::Error for SignalError;

// pub struct SignalingChannel {
//     tx: mpsc::Sender<Signal>,
//     rx: mpsc::Receiver<Signal>,
// }

// impl SignalingChannel {
//     pub fn pair(buffer: usize) -> (Self, Self) {
//         let (send1, recv1) = mpsc::channel(buffer);
//         let (send2, recv2) = mpsc::channel(buffer);

//         (Self {tx: send1, rx: recv2}, Self {tx: send2, rx: recv1})
//     }
//     pub fn send(&self, value: Signal) -> Result<(), SignalError> {
//         self.tx.try_send(value).map_err(|_| SignalError::BufferFull)
//     }
//     pub async fn recv(&mut self) -> Result<Signal, SignalError> {
//         self.rx.recv().await.ok_or(SignalError::ChannelClosed)
//     }
// }

