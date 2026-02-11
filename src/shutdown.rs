use tokio::sync::broadcast;

pub type ShutdownSender = broadcast::Sender<()>;
pub type ShutdownReceiver = broadcast::Receiver<()>;
