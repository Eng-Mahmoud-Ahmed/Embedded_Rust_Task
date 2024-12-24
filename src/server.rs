// Import the `EchoMessage` struct from the message module
use crate::message::EchoMessage;

// Import utilities for creating and managing TCP connections
use net2::{unix::UnixTcpBuilderExt, TcpBuilder};
// Import logging macros for structured logging
use log::{error, info, warn};
// Import threading utilities for creating and managing threads
use std::thread::{self, JoinHandle};
// Import duration struct for time-based operations
use std::time::Duration;
// Import prost for message encoding/decoding
use prost::Message;
// Import IO operations and networking utilities
use std::{
    io::{self, ErrorKind, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering}, // Atomic operations for thread-safe state
        Arc, // Shared ownership of data
    },
};
// Import synchronization primitives from parking_lot for performance
use parking_lot::{RwLock, Mutex};

// Represents a connected client
struct Client {
    stream: TcpStream, // Stream for client communication
}

impl Client {
    pub fn new(stream: TcpStream) -> Self {
        Client { stream }
    }

    // Handles communication with the client
    pub fn handle(&mut self) -> io::Result<()> {
        let mut buffer = [0u8; 1024]; // Buffer to store received data

        // Read data from the client
        let bytes_read: usize = self.stream.read(&mut buffer)?;

        if bytes_read == 0 {
            info!("Client disconnected."); // Log client disconnection
            return Ok(());
        }
        
        // Attempt to decode received data into an `EchoMessage`
        if let Ok(message) = EchoMessage::decode(&buffer[..bytes_read]) {
            info!("Received: {}", message.content); // Log received message
            // Encode and echo back the message
            let payload = message.encode_to_vec();
            self.stream.write_all(&payload)?;
            self.stream.flush()?;
        } else {
            println!("Server Received {} bytes", bytes_read); // Debug log
            error!("Failed to decode message"); // Log decoding error
        }

        Ok(())
    }
}

// Represents the server
pub struct Server {
    listener: Arc<TcpListener>, // Listener for incoming connections
    is_running: Arc<AtomicBool>, // Flag to track server state
    workers: Arc<Mutex<Vec<JoinHandle<()>>>>, // Worker threads
    client_handlers: Arc<RwLock<Vec<Arc<Mutex<Option<Client>>>>>>, // Client handlers
}

// Maximum number of clients the server can handle
const MAX_CLIENTS: usize = 100;

impl Server {
    // Constructor to initialize the server
    pub fn new(addr: &str) -> io::Result<Self> {
        // Create and configure a TCP listener
        let listener = TcpBuilder::new_v4()?
            .reuse_address(true)?
            .reuse_port(true)?
            .bind(addr)?
            .listen(42)?; // Backlog size

        Ok(Server {
            listener: Arc::new(listener),
            is_running: Arc::new(AtomicBool::new(false)),
            workers: Arc::new(Mutex::new(Vec::new())),
            client_handlers: Arc::new(RwLock::new(Vec::with_capacity(MAX_CLIENTS))),
        })
    }

    // Handles communication with a specific client
    fn handle_client(client: Arc<Mutex<Option<Client>>>, is_running: Arc<AtomicBool>) {
        while is_running.load(Ordering::SeqCst) { // Check if the server is running
            let mut client_guard = client.lock(); // Acquire lock on the client
            if let Some(ref mut client) = *client_guard {
                if let Err(e) = client.handle() { // Handle client communication
                    error!("Error handling client: {}", e); // Log error
                    break;
                }
            } else {
                break;
            }
            thread::sleep(Duration::from_millis(10)); // Prevent busy waiting
        }
    }

    // Starts the server and listens for connections
    pub fn run(&self) -> io::Result<()> {
        self.is_running.store(true, Ordering::SeqCst); // Set server state to running
        info!("Server is running on {}", self.listener.local_addr()?); // Log server start

        self.listener.set_nonblocking(true)?; // Set listener to non-blocking mode

        while self.is_running.load(Ordering::SeqCst) { // Main server loop
            match self.listener.accept() {
                Ok((stream, addr)) => {
                    info!("New client connected: {}", addr); // Log new connection
                    let client = Client::new(stream); // Create a new client
                    
                    let client_handler = Arc::new(Mutex::new(Some(client))); // Wrap client handler
                    let handler_clone = client_handler.clone();
                    let is_running = self.is_running.clone();

                    // Spawn a thread to handle the client
                    let handle = thread::spawn(move || {
                        Self::handle_client(handler_clone, is_running);
                    });

                    self.client_handlers.write().push(client_handler); // Add handler to list
                    self.workers.lock().push(handle); // Add thread to worker list
                }
                Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(100)); // Wait before retrying
                    continue;
                }
                Err(e) => {
                    error!("Error accepting connection: {}", e); // Log error
                }
            }
        }

        // Cleanup worker threads
        let mut workers = self.workers.lock();
        for handle in workers.drain(..) {
            if let Err(e) = handle.join() {
                error!("Error joining worker thread: {:?}", e); // Log thread join error
            }
        }

        info!("Server stopped."); // Log server stop
        Ok(())
    }

    // Stops the server
    pub fn stop(&self) {
        if self.is_running.load(Ordering::SeqCst) { // Check if the server is running
            self.is_running.store(false, Ordering::SeqCst); // Set server state to stopped
            
            // Clear all client handlers
            self.client_handlers.write().clear();
            
            info!("Shutdown signal sent."); // Log shutdown
        } else {
            warn!("Server was already stopped or not running."); // Log already stopped warning
        }
    }
}

// Clean up resources when the server is dropped
impl Drop for Server {
    fn drop(&mut self) {
        self.stop(); // Ensure the server is stopped
    }
}
