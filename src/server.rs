use crate::message::EchoMessage;

use net2::{unix::UnixTcpBuilderExt, TcpBuilder};
use log::{error, info, warn};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use prost::Message;
use std::{
    io::{self, ErrorKind, Read, Write}, net::{TcpListener, TcpStream}, sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    }
};
use parking_lot::RwLock;
use parking_lot::Mutex;


struct Client {
    stream: TcpStream,
}

impl Client {
    pub fn new(stream: TcpStream) -> Self {
        Client { stream }
    }

    pub fn handle(&mut self) -> io::Result<()> {
        let mut buffer = [0u8; 1024];

        // Read data from the client
        let bytes_read: usize = self.stream.read(&mut buffer)?;

        if bytes_read == 0 {
            info!("Client disconnected.");
            return Ok(());
        }
        
        if let Ok(message) = EchoMessage::decode(&buffer[..bytes_read]) {
            info!("Received: {}", message.content);
            // Echo back the message
            let payload = message.encode_to_vec();
            self.stream.write_all(&payload)?;
            self.stream.flush()?
        } else {
            println!("Server Received {} bytes", bytes_read);
            error!("Failed to decode message");
        }

        Ok(())
    }
}

pub struct Server {
    listener: Arc<TcpListener>,
    is_running: Arc<AtomicBool>,
    workers: Arc<Mutex<Vec<JoinHandle<()>>>>,
    client_handlers: Arc<RwLock<Vec<Arc<Mutex<Option<Client>>>>>>,
}

const MAX_CLIENTS: usize = 100;

impl Server {
    pub fn new(addr: &str) -> io::Result<Self> {
        let listener = TcpBuilder::new_v4()?
            .reuse_address(true)?
            .reuse_port(true)?
            .bind(addr)?
            .listen(42)?;

        Ok(Server {
            listener: Arc::new(listener),
            is_running: Arc::new(AtomicBool::new(false)),
            workers: Arc::new(Mutex::new(Vec::new())),
            client_handlers: Arc::new(RwLock::new(Vec::with_capacity(MAX_CLIENTS))),
        })
    }

    fn handle_client(client: Arc<Mutex<Option<Client>>>, is_running: Arc<AtomicBool>) {
        while is_running.load(Ordering::SeqCst) {
            let mut client_guard = client.lock();
            if let Some(ref mut client) = *client_guard {
                if let Err(e) = client.handle() {
                    error!("Error handling client: {}", e);
                    break;
                }
            } else {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn run(&self) -> io::Result<()> {
        self.is_running.store(true, Ordering::SeqCst);
        info!("Server is running on {}", self.listener.local_addr()?);

        self.listener.set_nonblocking(true)?;

        while self.is_running.load(Ordering::SeqCst) {
            match self.listener.accept() {
                Ok((stream, addr)) => {
                    info!("New client connected: {}", addr);
                    let client = Client::new(stream);
                    
                    let client_handler = Arc::new(Mutex::new(Some(client)));
                    let handler_clone = client_handler.clone();
                    let is_running = self.is_running.clone();

                    let handle = thread::spawn(move || {
                        Self::handle_client(handler_clone, is_running);
                    });

                    self.client_handlers.write().push(client_handler);
                    self.workers.lock().push(handle);
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(100));
                    continue;
                }
                Err(e) => {
                    error!("Error accepting connection: {}", e);
                }
            }
        }

        // Cleanup
        let mut workers = self.workers.lock();
        for handle in workers.drain(..) {
            if let Err(e) = handle.join() {
                error!("Error joining worker thread: {:?}", e);
            }
        }

        info!("Server stopped.");
        Ok(())
    }

    pub fn stop(&self) {
        if self.is_running.load(Ordering::SeqCst) {
            self.is_running.store(false, Ordering::SeqCst);
            
            // Clear all client handlers
            self.client_handlers.write().clear();
            
            info!("Shutdown signal sent.");
        } else {
            warn!("Server was already stopped or not running.");
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
    }
}